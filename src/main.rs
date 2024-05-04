use std::path::PathBuf;

use anyhow::Result as AResult;

use clap::Parser;

use liso::{InputOutput, liso, Response};

use vinezombie::client;
use vinezombie::client::handlers;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Connect with TLS
    #[arg(long)]
    tls: bool,
    /// Don't verify the server's TLS certificate
    #[arg(long)]
    noverify: bool,
    /// Client certificate for CertFP
    #[arg(long, value_name = "FILE")]
    cert: Option<PathBuf>,
    /// Log raw traffic to a file
    #[arg(short, long, value_name = "FILE")]
    log: Option<PathBuf>,
    /// Reconnect on connection close after a pause
    #[arg(long, value_name = "TIME")]
    reconnect: Option<String>,
    /// Disable automatic ping responses
    #[arg(short, long)]
    noping: bool,
    /// Server to connect to
    host: String,
    /// Port to use when connecting to the server
    port: Option<String>,
}

#[tokio::main]
async fn main() -> AResult<()> {
    let cli = Cli::parse();

    let mut io = InputOutput::new();
    let out = io.clone_output();
    io.prompt(liso!(fg=green, bold, "-> ", reset), true, false);

    let host: vinezombie::string::Word = cli.host.as_str().try_into()?;
    let address = client::conn::ServerAddr::from_host(host)?;

    let tls_options = client::tls::TlsConfigOptions {
        trust: if cli.noverify { client::tls::Trust::NoVerify } else { client::tls::Trust::Default },
        cert: cli.cert,
    };
    let sock = address.connect_tokio(|| tls_options.build()).await?;

    let mut client = client::Client::new(sock, client::channel::TokioChannels);

    let _ = client.add((), handlers::AutoPong);
    let mut queue = client.queue_mut().edit();
    let (_, mut msgs) = client.add((), handlers::YieldAll).unwrap();

    tokio::spawn(async move {
        while let Some(msg) = msgs.recv().await {
            out.println(format!("{msg}"));
        }
    });

    tokio::spawn(async move {
        loop {
            let _ = client.run_tokio().await;
        }
    });

    'input: loop {
        match io.read_async().await {
            Response::Quit | Response::Finish => {
                // send QUIT
                break 'input;
            },
            Response::Input(s) => {
                let msg = vinezombie::ircmsg::ClientMsg::parse(s.clone()).unwrap();
                queue.push(msg);
                io.echoln(liso!(fg=green, bold, "-> ", reset, s));
            },
            _ => (),
        };
    }

    Ok(())
}
