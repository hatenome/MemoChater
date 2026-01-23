//! Qdrant 嵌入式进程管理
//! 
//! 将 Qdrant 可执行文件作为子进程启动和管理

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tokio::time::sleep;

/// Qdrant 进程管理器
pub struct QdrantManager {
    /// Qdrant 可执行文件路径
    exe_path: PathBuf,
    /// 存储目录
    storage_path: PathBuf,
    /// 监听端口
    port: u16,
    /// 子进程句柄
    child: Option<Child>,
}

impl QdrantManager {
    /// 创建新的 Qdrant 管理器
    pub fn new(exe_path: PathBuf, storage_path: PathBuf, port: u16) -> Self {
        Self {
            exe_path,
            storage_path,
            port,
            child: None,
        }
    }

    /// 启动 Qdrant 进程
    pub async fn start(&mut self) -> Result<(), QdrantError> {
        // 检查可执行文件是否存在
        if !self.exe_path.exists() {
            return Err(QdrantError::ExeNotFound(self.exe_path.display().to_string()));
        }

        // 确保存储目录存在
        if !self.storage_path.exists() {
            std::fs::create_dir_all(&self.storage_path)
                .map_err(|e| QdrantError::IoError(e.to_string()))?;
        }

        // 获取 exe 所在目录作为工作目录
        let working_dir = self.exe_path.parent()
            .ok_or_else(|| QdrantError::IoError("无法获取 Qdrant 工作目录".to_string()))?;

        tracing::info!(
            "启动 Qdrant: exe={}, storage={}, port={}, working_dir={}",
            self.exe_path.display(),
            self.storage_path.display(),
            self.port,
            working_dir.display()
        );

        // 使用环境变量配置 Qdrant
        // 参考: https://qdrant.tech/documentation/guides/configuration/
        let storage_path_str = self.storage_path.canonicalize()
            .unwrap_or_else(|_| self.storage_path.clone())
            .display()
            .to_string();

        let child = Command::new(&self.exe_path)
            .current_dir(working_dir)
            .env("QDRANT__SERVICE__HTTP_PORT", self.port.to_string())
            .env("QDRANT__SERVICE__GRPC_PORT", (self.port + 1).to_string())
            .env("QDRANT__STORAGE__STORAGE_PATH", &storage_path_str)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| QdrantError::SpawnError(e.to_string()))?;

        self.child = Some(child);

        // 等待 Qdrant 就绪
        self.wait_for_ready().await?;

        tracing::info!("Qdrant 启动成功，监听端口: {}", self.port);
        Ok(())
    }

    /// 等待 Qdrant 服务就绪
    async fn wait_for_ready(&self) -> Result<(), QdrantError> {
        let url = format!("http://127.0.0.1:{}/readyz", self.port);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .no_proxy()
            .build()
            .map_err(|e| QdrantError::ConnectionError(e.to_string()))?;

        let max_attempts = 30; // 最多等待 30 秒
        for attempt in 1..=max_attempts {
            match client.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    tracing::debug!("Qdrant 健康检查通过 (尝试 {})", attempt);
                    return Ok(());
                }
                Ok(resp) => {
                    tracing::debug!(
                        "Qdrant 健康检查返回非成功状态: {} (尝试 {})",
                        resp.status(),
                        attempt
                    );
                }
                Err(e) => {
                    tracing::debug!("Qdrant 健康检查失败: {} (尝试 {})", e, attempt);
                }
            }
            sleep(Duration::from_secs(1)).await;
        }

        Err(QdrantError::StartupTimeout)
    }

    /// 停止 Qdrant 进程
    pub fn stop(&mut self) -> Result<(), QdrantError> {
        if let Some(mut child) = self.child.take() {
            tracing::info!("正在停止 Qdrant 进程...");
            
            // 尝试优雅关闭
            #[cfg(windows)]
            {
                // Windows 上使用 taskkill
                let _ = Command::new("taskkill")
                    .args(["/PID", &child.id().to_string(), "/F"])
                    .output();
            }
            
            #[cfg(not(windows))]
            {
                // Unix 上发送 SIGTERM
                let _ = child.kill();
            }

            // 等待进程退出
            match child.wait() {
                Ok(status) => {
                    tracing::info!("Qdrant 进程已退出: {}", status);
                }
                Err(e) => {
                    tracing::warn!("等待 Qdrant 进程退出失败: {}", e);
                }
            }
        }
        Ok(())
    }

    /// 获取 Qdrant URL
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// 检查进程是否还在运行
    pub fn is_running(&mut self) -> bool {
        if let Some(ref mut child) = self.child {
            match child.try_wait() {
                Ok(None) => true,  // 进程还在运行
                Ok(Some(_)) => false,  // 进程已退出
                Err(_) => false,
            }
        } else {
            false
        }
    }
}

impl Drop for QdrantManager {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// Qdrant 错误类型
#[derive(Debug, thiserror::Error)]
pub enum QdrantError {
    #[error("Qdrant 可执行文件不存在: {0}")]
    ExeNotFound(String),

    #[error("IO 错误: {0}")]
    IoError(String),

    #[error("启动 Qdrant 进程失败: {0}")]
    SpawnError(String),

    #[error("连接 Qdrant 失败: {0}")]
    ConnectionError(String),

    #[error("Qdrant 启动超时")]
    StartupTimeout,
}