# MemoChater WebUI

MemoChater 的 Web 管理界面，基于 Vue 3 + TypeScript + TailwindCSS 构建。

## 功能特性

- 🤖 **助手管理** - 创建、编辑、删除 AI 助手
- 💬 **对话界面** - 与助手进行流式对话
- 📝 **话题管理** - 管理对话话题和历史记录
- 🧠 **记忆管理** - 查看、搜索、创建长期记忆
- 📋 **待处理池** - 管理待处理的记忆条目

## 技术栈

- **框架**: Vue 3 (Composition API)
- **语言**: TypeScript
- **状态管理**: Pinia
- **路由**: Vue Router
- **样式**: TailwindCSS
- **构建工具**: Vite

## 快速开始

### 1. 安装依赖

```bash
cd G:\MemoChater\webui
npm install
```

### 2. 启动开发服务器

```bash
npm run dev
```

开发服务器将在 http://localhost:5173 启动，并自动代理 API 请求到后端。

### 3. 启动后端服务

在另一个终端中：

```bash
cd G:\MemoChater\memo-chater
cargo run
```

后端服务将在 http://localhost:7892 启动。

### 4. 启动 Qdrant（如果使用外部模式）

如果配置为外部 Qdrant 模式，需要先启动 Qdrant：

```bash
cd G:\MemoChater\qdrant-x86_64-pc-windows-msvc
.\qdrant.exe
```

## 构建生产版本

```bash
npm run build
```

构建产物将输出到 `dist` 目录。

## 项目结构

```
webui/
├── src/
│   ├── api/           # API 客户端
│   │   ├── client.ts      # 基础请求封装
│   │   ├── assistants.ts  # 助手 API
│   │   ├── memory.ts      # 记忆 API
│   │   └── chat.ts        # 对话 API
│   ├── components/    # 通用组件
│   │   ├── Sidebar.vue    # 侧边栏
│   │   ├── Toast.vue      # 消息提示
│   │   ├── ChatInput.vue  # 聊天输入框
│   │   └── ChatMessage.vue # 消息气泡
│   ├── stores/        # Pinia 状态管理
│   │   ├── app.ts         # 应用状态
│   │   └── assistant.ts   # 助手状态
│   ├── types/         # TypeScript 类型定义
│   ├── views/         # 页面视图
│   │   ├── ChatView.vue       # 对话页面
│   │   ├── AssistantsView.vue # 助手管理
│   │   ├── MemoryView.vue     # 记忆管理
│   │   └── SettingsView.vue   # 设置页面
│   ├── App.vue        # 根组件
│   ├── main.ts        # 入口文件
│   ├── router.ts      # 路由配置
│   └── style.css      # 全局样式
├── public/            # 静态资源
├── index.html         # HTML 模板
├── package.json       # 依赖配置
├── vite.config.ts     # Vite 配置
├── tailwind.config.js # TailwindCSS 配置
└── tsconfig.json      # TypeScript 配置
```

## API 代理配置

开发模式下，Vite 会将 `/api` 前缀的请求代理到后端：

```typescript
// vite.config.ts
server: {
  proxy: {
    '/api': {
      target: 'http://localhost:7892',
      changeOrigin: true,
      rewrite: (path) => path.replace(/^\/api/, '')
    }
  }
}
```

## 注意事项

1. 确保后端服务已启动并监听在 7892 端口
2. 确保 Qdrant 服务可用（内嵌或外部模式）
3. 首次使用需要创建助手和话题才能开始对话