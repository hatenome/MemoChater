//! 存储模块
//! 
//! 提供文件内容的持久化存储（JSON文件，无C依赖）

mod file_store;

pub use file_store::*;