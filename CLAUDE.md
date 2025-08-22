# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

这是一个基于 Electron 的浏览器项目内核，使用 napi-rs 实现系统交互功能并导出给 Electron 项目调用。该项目是一个 Rust cdylib 库，通过 Node.js N-API 绑定提供本地扩展功能。

## 主要功能模块

- **书签管理**: 使用 SQLite 存储和管理书签数据 (`src/store/bookmark.rs`)
- **浏览历史**: 记录和查询浏览历史数据 (`src/store/history.rs`)  
- **下载历史**: 管理文件下载记录 (`src/store/download.rs`)
- **网站图标**: 将 favicon 解析为 base64 格式存储到本地数据库 (`src/store/favicon.rs`)
- **配置存储**: 使用 SQLite 保存应用配置数据

## 核心架构

### 存储层架构 (`src/store/mod.rs`)

- **全局路径管理**: 使用 `OnceLock<String>` 存储数据库基础路径
- **连接池管理**: 通过 `Arc<Mutex<Connection>>` 管理 SQLite 连接
- **数据库操作**: 提供统一的查询、执行和事务操作接口
  - `query_simple`: 简单查询操作
  - `execute_simple`: 简单执行操作  
  - `execute_transaction`: 事务操作

### 初始化流程

1. 调用 `store_init(db_path)` 设置数据库路径
2. 依次初始化历史、书签、下载数据库表结构
3. 所有模块共享同一个数据库基础路径

## 常用命令

### 构建和开发

```bash
# 构建 release 版本
npm run build

# 使用构建脚本（移动文件到 dist 目录）
./build.sh

# 直接使用 napi 构建
napi build --platform --release
```

### 支持平台

- Windows: x86_64-pc-windows-msvc, aarch64-pc-windows-msvc
- macOS: x86_64-apple-darwin, aarch64-apple-darwin

## 依赖管理

### 核心依赖

- `napi`: Node.js N-API 绑定
- `napi-derive`: 宏支持
- `rusqlite`: SQLite 数据库操作
- `sea-query`: SQL 查询构建器
- `serde`: 序列化支持
- `anyhow`: 错误处理

### 开发注意事项

1. 对项目结构重大变更前需要先进行设计确认
2. 修改代码前必须阅读当前代码，避免随意变动已有代码
3. 每次对话专注解决一个问题
4. 执行任务前需重新阅读相关代码，不要修改与当前任务无关的代码  
5. 不允许修改现有的方法名、变量名、类名、函数名、接口名，除非必要

## 代码规范

- 使用 `#![deny(clippy::all)]` 确保代码质量
- 所有公共接口通过 `#[napi]` 宏导出
- 错误处理统一使用 `anyhow::Error`
- 数据库操作统一通过 store 模块的抽象接口