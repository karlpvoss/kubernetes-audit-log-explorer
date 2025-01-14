use crossterm::{self, event::EventStream};
use futures::stream::StreamExt;
use kubernetes_audit_log_explorer::{kube::EventV1, App};
use std::io::stdin;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut app = App::new();
    app.setup();

    // read and process log events from /dev/stdin
    let (send, mut recv) = mpsc::unbounded_channel();
    tokio::task::spawn_blocking(|| stdin_processor(send));
    // read and process terminal events from /dev/tty
    let mut terminal_events = EventStream::new();

    app.draw();

    let mut interval = tokio::time::interval(std::time::Duration::from_millis(100));
    loop {
        let stdin_event = recv.recv();
        let term_event = terminal_events.next();

        tokio::select! {
            Some(maybe_kube_event) = stdin_event => {
                match maybe_kube_event {
                    Ok(kube_event) => app.handle_kube_event(kube_event),
                    Err(error) => app.set_error(error),
                }
            },
            maybe_event = term_event => {
                match maybe_event {
                    Some(event) => {
                        if app.handle_terminal_event(event).is_some() {
                            break;
                        }
                    }
                    None => break,
                }
                app.draw();
            },
            _ = interval.tick() => {
                app.draw();
            },
        };
    }

    app.tear_down();
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
fn stdin_processor(send: mpsc::UnboundedSender<anyhow::Result<EventV1>>) -> anyhow::Result<()> {
    let stdin = stdin();
    let stream = serde_json::Deserializer::from_reader(stdin).into_iter::<EventV1>();

    for maybe_event in stream {
        match maybe_event {
            Ok(event) => {
                // Drop events that don't refer to things in the cluster
                if !(event.request_uri.starts_with("/api/")
                    || event.request_uri.starts_with("/apis/"))
                {
                    continue;
                }

                send.send(Ok(event))?;
            }
            Err(err) => {
                send.send(Err(err.into()))?;
                anyhow::bail!("stdin failed to deserialise, quitting")
            }
        }
    }

    send.send(Err(anyhow::anyhow!("reached the end of stdin")))?;

    Ok(())
}
