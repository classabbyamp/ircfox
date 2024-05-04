use std::path::PathBuf;

use anyhow::Result as AResult;

use clap::Parser;

use crossterm::style::Stylize;

use liso::{InputOutput, liso, Response};

use vinezombie::{client::{self, handlers, tls::{TlsConfigOptions, Trust}}, ircmsg::ServerMsg, names::cmd};
use vinezombie::ircmsg::ClientMsg;
use vinezombie::string as vzstr;

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

// fn format_clientmsg(msg: ClientMsg) -> String {
//     
// }
//
// fn format_servermsg(msg: ServerMsg) -> String {
//     format!("{} {} {} {}", msg.tags.red())
// }

#[tokio::main]
async fn main() -> AResult<()> {
    let cli = Cli::parse();

    let mut io = InputOutput::new();
    let out = io.clone_output();
    io.prompt(liso!(fg=green, bold, "-> ", reset), true, false);

    let host: vzstr::Word = cli.host.as_str().try_into()?;
    let address = client::conn::ServerAddr::from_host(host)?;

    let mut tls_opt = TlsConfigOptions::default();
    tls_opt.cert = cli.cert;
    if cli.noverify {
        tls_opt.trust = Trust::NoVerify;
    };

    let sock = address.connect_tokio(|| tls_opt.build()).await?;

    let mut client = client::Client::new(sock, client::channel::TokioChannels);

    let _ = client.add((), handlers::AutoPong);
    let (_, mut msgs) = client.add((), handlers::YieldAll).unwrap();

    let mut got_err = false;
    let res = loop {
        tokio::select! {
            biased;
            Some(msg) = msgs.recv() => {
                out.println(format!("{msg}"));

                if msg.kind == cmd::ERROR {
                    got_err = true;
                    break Ok(());
                }
            }
            res = client.run_tokio() => {
                if let Err(e) = res {
                    break Err(e);
                }
            }
            input = io.read_async() => {
                match input {
                    Response::Quit | Response::Finish | Response::Dead => {
                        let mut m = ClientMsg::new(cmd::QUIT);
                        m.args.edit().add(vzstr::Arg::from_str("goodbye"));
                        client.queue_mut().edit().push(m.clone());
                        io.echoln(liso!(fg=green, bold, "-> ", reset, format!("{m}")));
                    },
                    Response::Input(s) => {
                        if s.len() > 0 {
                            match ClientMsg::parse(s.clone()) {
                                Ok(m) => {
                                    client.queue_mut().edit().push(m);
                                    io.echoln(liso!(fg=green, bold, "-> ", reset, s));
                                },
                                Err(e) => io.echoln(liso!(fg=red, bold, "-> [", e.to_string(), "] ", reset, s)),
                            }
                        }
                    },
                    _ => (),
                }
            }

        }
    };

    io.die().await;
    std::mem::drop(client);

    while let Some(msg) = msgs.recv().await {
        if msg.kind == cmd::ERROR {
            got_err = true;
        }
        out.println(format!("{msg}"));
    }

    if got_err {
        Ok(())
    } else {
        //res.map_err(todo!())
        todo!()
    }
}
