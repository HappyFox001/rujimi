use std::process::Command;
use std::env;
use tokio::time::{sleep, Duration};
use tracing::{info, warn, error};

pub fn open_browser_with_port(port: u16) {
    // 检查是否在无 GUI 的 Linux 环境中（但不检查 macOS）
    if cfg!(target_os = "linux") && env::var("DISPLAY").is_err() {
        info!("检测到无 GUI 环境 (缺少 DISPLAY 环境变量)，跳过打开浏览器。");
        return;
    }

    let url = format!("http://127.0.0.1:{}", port);

    let result = if cfg!(target_os = "windows") {
        // Windows
        Command::new("cmd")
            .args(&["/C", "start", &url])
            .spawn()
    } else if cfg!(target_os = "macos") {
        // macOS
        Command::new("open")
            .arg(&url)
            .spawn()
    } else {
        // Linux/Unix
        Command::new("xdg-open")
            .arg(&url)
            .spawn()
    };

    match result {
        Ok(mut child) => {
            match child.wait() {
                Ok(status) if status.success() => {
                    info!("已发送打开浏览器指令: {}", url);
                },
                Ok(_) => {
                    warn!("浏览器命令执行失败，但无错误信息");
                },
                Err(e) => {
                    warn!("等待浏览器进程时出错: {}", e);
                }
            }
        },
        Err(e) => {
            warn!("系统中未找到可用的浏览器，跳过自动打开: {}", e);
        }
    }
}

pub fn open_browser() {
    open_browser_with_port(7860);
}

pub async fn open_browser_delayed() {
    open_browser_delayed_with_port(7860).await;
}

pub async fn open_browser_delayed_with_port(port: u16) {
    info!("将在3秒后自动打开浏览器...");
    sleep(Duration::from_secs(3)).await;

    tokio::task::spawn_blocking(move || {
        open_browser_with_port(port);
    }).await.unwrap_or_else(|e| {
        error!("打开浏览器任务执行失败: {}", e);
    });
}