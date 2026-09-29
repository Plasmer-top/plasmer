use discord_rich_presence::{DiscordIpc, DiscordIpcClient, activity};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

const RPC_ENABLED: bool = true;

const RECONNECT_INTERVAL: Duration = Duration::from_secs(10);
const COMMAND_POLL_INTERVAL: Duration = Duration::from_millis(500);

pub(crate) struct DiscordRpc {
  tx: Option<Sender<RpcCommand>>,
}

#[derive(Clone, Copy)]
enum RpcState {
  Idling,
  Cps(u32),
}

enum RpcCommand {
  SetState(RpcState),
  Shutdown,
}

const CLIENT_ID: &str = "1492218027205460199";
const DISCORD_INVITE_URL: &str = "https://plasmer.top/api/discord";

impl DiscordRpc {
  pub(crate) fn new(app_version: u32, _cps: u32) -> Self {
    if !RPC_ENABLED {
      return Self { tx: None };
    }

    let (tx, rx) = mpsc::channel::<RpcCommand>();

    let _ = thread::Builder::new()
      .name("discord-rpc".into())
      .spawn(move || {
        let mut rpc_state = RpcState::Idling;
        let mut client: Option<DiscordIpcClient> = None;
        let mut last_connect_attempt = Instant::now() - RECONNECT_INTERVAL;

        loop {
          if client.is_none() && last_connect_attempt.elapsed() >= RECONNECT_INTERVAL {
            last_connect_attempt = Instant::now();

            let mut next = DiscordIpcClient::new(CLIENT_ID);
            if next.connect().is_ok() {
              let _ = set_activity(&mut next, app_version, rpc_state);
              client = Some(next);
            }
          }

          match rx.recv_timeout(COMMAND_POLL_INTERVAL) {
            Ok(RpcCommand::SetState(next_state)) => {
              rpc_state = next_state;
              if let Some(active) = client.as_mut() {
                if set_activity(active, app_version, rpc_state).is_err() {
                  let _ = active.close();
                  client = None;
                }
              }
            }
            Ok(RpcCommand::Shutdown) => break,
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
          }
        }

        if let Some(mut active) = client {
          let _ = active.clear_activity();
          let _ = active.close();
        }
      });

    Self { tx: Some(tx) }
  }

  pub(crate) fn update(&mut self) {}

  pub(crate) fn set_idling(&mut self) {
    self.send(RpcCommand::SetState(RpcState::Idling));
  }

  pub(crate) fn set_cps_on_start(&mut self, cps: u32) {
    self.send(RpcCommand::SetState(RpcState::Cps(cps)));
  }

  pub(crate) fn shutdown(&mut self) {
    self.send(RpcCommand::Shutdown);
    self.tx = None;
  }

  fn send(&self, command: RpcCommand) {
    if let Some(tx) = &self.tx {
      let _ = tx.send(command);
    }
  }
}

impl Drop for DiscordRpc {
  fn drop(&mut self) {
    self.shutdown();
  }
}

fn set_activity(
  client: &mut DiscordIpcClient,
  app_version: u32,
  state: RpcState,
) -> Result<(), ()> {
  let activity = activity::Activity::new()
    .details(format!("Plasmer Macro v{}", app_version))
    .state(match state {
      RpcState::Idling => "Idling".to_string(),
      RpcState::Cps(cps) => format!("CPS: [{}]", cps),
    })
    .buttons(vec![activity::Button::new(
      "Get Plasmer",
      DISCORD_INVITE_URL,
    )])
    .assets(
      activity::Assets::new()
        .large_image("plasmer_500")
        .large_text("Plasmer"),
    );

  client.set_activity(activity).map_err(|_| ())
}
