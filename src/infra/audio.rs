//! Simple audio playback for new message notification.

/// 播放新消息提示音 (使用内嵌资源)
/// 收到非自己的消息时调用
pub fn play_notification() {
    // 尝试播放通知音，如果失败则静默忽略
    // rodio 需要系统音频支持，此处为占位实现
    // 第二阶段可通过 rodio crate 实现真实音频播放
    crate::log_info!("[audio] 新消息提示音 (占位)");
}
