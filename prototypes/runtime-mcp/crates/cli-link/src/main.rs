//! Runs the unmodified `airs` entry point; the hidden `__mcp-probe URL` argument
//! keeps the MCP client code reachable so the linker cannot drop it, giving an
//! honest upper-bound size delta for compiling the client into the CLI.

fn main() {
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    if args.get(1).and_then(|a| a.to_str()) == Some("__mcp-probe") {
        let url = args
            .get(2)
            .and_then(|a| a.to_str())
            .unwrap_or("http://127.0.0.1:9/mcp")
            .to_owned();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let outcome = rt.block_on(airs_mcp_client::call_tool(
            &url,
            None,
            "service_info",
            Default::default(),
        ));
        println!("{}", outcome.is_ok());
        println!("{}", airs_mcp_client::core_sdk_pin());
        return;
    }
    std::process::exit(airs_cli_rs::run(args));
}
