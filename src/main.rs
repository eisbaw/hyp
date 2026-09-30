#[tokio::main]
async fn main() {
    let cli = hyp::cli::Cli::parse_checked();
    if !matches!(cli.command, hyp::cli::Command::Web { .. }) {
        restore_default_sigpipe();
    }
    let json = cli.json;
    if let Err(err) = hyp::cli::run(cli).await {
        if json {
            eprintln!("{}", hyp::error::to_json(&err));
        } else {
            eprintln!("hyp: {err:#}");
        }
        std::process::exit(hyp::cli::exit_code(&err));
    }
}
/// Rust ignores SIGPIPE, so `println!` into a closed pipe (`hyp list | head`)
/// panics with exit 101. With the default disposition hyp ends quietly like
/// other Unix filters (shell status 141). This is simpler and harder to regress
/// than handling BrokenPipe at every print site.
///
/// `hyp web` keeps SIGPIPE ignored: hyper writes responses with writev(2),
/// which, unlike std's send(MSG_NOSIGNAL), raises SIGPIPE on a connection the
/// peer has reset. A disconnecting browser must not be able to kill the server.
#[cfg(unix)]
fn restore_default_sigpipe() {
    // SAFETY: only changes the process-wide disposition to the kernel default;
    // no Rust code runs as a signal handler.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}
#[cfg(not(unix))]
fn restore_default_sigpipe() {}
