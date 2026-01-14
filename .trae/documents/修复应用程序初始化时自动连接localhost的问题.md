# 修复应用程序初始化时自动连接localhost的问题

## 问题分析

应用程序启动后，用户没有输入任何信息，但出现了"Could not connect to localhost: Connection refused"错误。经过分析，发现问题在于：

1. 应用程序初始化时创建了`DefaultConnectionManager`实例
2. 存在逻辑问题，可能导致在没有用户输入的情况下尝试连接
3. 主机名解析逻辑有缺陷，可能将空主机名默认解析为localhost

## 修复方案

### 1. 增强连接管理器初始化

- 确保`DefaultConnectionManager`在初始化时不自动创建连接
- 验证`connections`、`sessions`和`ssh_handles`在启动时为空

### 2. 改进主机名解析逻辑

修改`ssh_impl.rs`中的`connect_ssh`函数，添加正确的主机名解析：

```rust
// 添加主机名解析逻辑
let addr = {
    // 尝试直接解析为IP地址
    if let Ok(addr) = SocketAddr::from_str(&format!("{}:{}", host, port)) {
        addr
    } else {
        // 主机名解析
        let addrs = tokio::net::lookup_host((host, port)).await?;
        addrs.into_iter().next().ok_or_else(|| anyhow!("Failed to resolve host: {}", host))?
    }
};
```

### 3. 增强配置验证

- 确保`validate`方法在所有连接尝试前被正确调用
- 添加更严格的验证，确保只有在用户明确输入后才尝试连接

### 4. 添加详细日志

在连接相关的关键位置添加日志，帮助调试和定位问题：

- 连接尝试的开始和结束
- 配置验证的结果
- 连接错误的详细信息

### 5. 前端改进建议

- 确保前端只有在用户填写了所有必要信息后才调用连接API
- 隐藏或禁用连接按钮，直到所有必填字段都已填写

## 预期效果

- 应用程序启动后不会自动尝试连接任何主机
- 只有在用户明确输入连接信息并触发连接操作后，才会尝试连接
- 连接错误信息更加详细，有助于用户理解问题
- 提高应用程序的稳定性和用户体验

## 实现步骤

1. 修改`ssh_impl.rs`，添加正确的主机名解析逻辑
2. 增强`connect`方法的验证和日志
3. 确保连接管理器初始化时不自动创建连接
4. 测试修复后的应用程序，验证问题是否解决
