use crate::{
    map,
    mutex::{Mutex, RwLock},
    sql,
    user::User,
    Action, BlockData, BlockInfo, Error,
};
use pso2packetlib::{connection::ConnectionError, PrivateKey};
use std::{
    io,
    sync::{atomic::AtomicU32, Arc},
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc,
};

pub async fn init_block(
    blocks: Arc<RwLock<Vec<BlockInfo>>>,
    this_block: BlockInfo,
    sql: Arc<sql::Sql>,
    key: PrivateKey,
) -> Result<(), Error> {
    let listener = TcpListener::bind(("0.0.0.0", this_block.port)).await?;

    let latest_mapid = AtomicU32::new(1);

    let Some(lobby) = this_block.server_data.maps.get(&this_block.lobby_map) else {
        return Err(Error::NoMapFound(this_block.lobby_map.clone()));
    };

    let lobby = Arc::new(Mutex::new({
        let mut map = map::Map::new_from_data(lobby.clone(), &latest_mapid)?;
        map.set_map_type(map::MapType::Lobby);
        map
    }));

    let block_data = Arc::new(BlockData {
        sql,
        blocks,
        block_id: this_block.id,
        block_name: this_block.name,
        lobby,
        key,
        latest_mapid,
        latest_partyid: AtomicU32::new(1),
        server_data: this_block.server_data,
        quests: this_block.quests,
        clients: Mutex::new(vec![]),
    });
    // we are the only owner of the map, so this never blocks
    block_data
        .lobby
        .lock_blocking()
        .set_block_data(block_data.clone());

    let mut conn_id = 0usize;
    let (send, mut recv) = mpsc::channel(10);

    loop {
        tokio::select! {
            // we opt out of random selection because the listener is rarely accepting
            biased;
            result = listener.accept() => {
                let (stream, _) = result?;
                new_conn_handler(
                    stream,
                    &block_data,
                    send.clone(),
                    this_block.id,
                    &mut conn_id,
                )
                .await?;
            }
            Some((id, action)) = recv.recv() => {
                match run_action(&block_data, id, action, &block_data).await {
                    Ok(_) => {}
                    Err(e) => log::warn!("Client error: {e}"),
                };
            }
        };
    }
}

async fn new_conn_handler(
    s: TcpStream,
    block_data: &Arc<BlockData>,
    send: mpsc::Sender<(usize, Action)>,
    block_id: u32,
    conn_id_ref: &mut usize,
) -> Result<(), Error> {
    log::info!("Client connected");

    let mut lock = block_data.blocks.write().await;
    if let Some(block) = lock.iter_mut().find(|x| x.id == block_id) {
        if block.players >= block.max_players {
            return Ok(());
        }
        block.players += 1;
    }
    drop(lock);

    let conn_id = *conn_id_ref;
    let (client, mut read) = User::new(s, block_data.clone(), conn_id)?;
    let client = Arc::new(Mutex::new(client));
    let mut clients = block_data.clients.lock().await;
    clients.push((conn_id, client.clone()));
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(100));
        loop {
            let result = tokio::select! {
                biased;
                result = read.read_packet_async() => {
                    match result {
                        Ok(a) => {
                            let lock = client.lock().await;
                            crate::user::packet_handler(lock, a).await
                        },
                        Err(ConnectionError::Io(e)) if matches!(e.kind(), io::ErrorKind::Interrupted) => Ok(Action::Nothing),
                        Err(ConnectionError::Io(e))
                            if matches!(
                                e.kind(),
                                io::ErrorKind::ConnectionAborted | io::ErrorKind::ConnectionReset
                            ) =>
                        {
                            send.send((conn_id, Action::Disconnect)).await.unwrap();
                            return;
                        }
                        Err(e) => {
                            let error_msg = format!("Client error: {e}");
                            let _ = client.lock().await.send_error(&error_msg).await;
                            log::warn!("{}", error_msg);
                            Ok(Action::Nothing)
                        }
                    }
                }
                _ = interval.tick() => {
                    User::tick(client.lock().await).await
                }
            };
            match result {
                Ok(Action::Nothing) => {}
                Ok(Action::Disconnect) => {
                    send.send((conn_id, Action::Disconnect)).await.unwrap();
                    return;
                }
                Err(Error::IOError(e)) if matches!(e.kind(), io::ErrorKind::Interrupted) => {}
                Err(Error::IOError(e))
                    if matches!(
                        e.kind(),
                        io::ErrorKind::ConnectionAborted | io::ErrorKind::ConnectionReset
                    ) =>
                {
                    send.send((conn_id, Action::Disconnect)).await.unwrap();
                    return;
                }
                Err(e) => {
                    let error_msg = format!("Client error: {e}");
                    let _ = client.lock().await.send_error(&error_msg).await;
                    log::warn!("{}", error_msg);
                }
            }
        }
    });

    *conn_id_ref += 1;
    Ok(())
}

async fn run_action(
    block: &BlockData,
    conn_id: usize,
    action: Action,
    block_data: &Arc<BlockData>,
) -> Result<(), Error> {
    if let Action::Nothing = action {
        return Ok(());
    }
    let mut clients = block.clients.lock().await;
    let Some((pos, _)) = clients
        .iter()
        .enumerate()
        .find(|(_, (c_conn_id, _))| *c_conn_id == conn_id)
    else {
        return Ok(());
    };
    match action {
        Action::Nothing => {}
        Action::Disconnect => {
            log::info!("Client disconnected");
            clients.remove(pos);

            let mut lock = block_data.blocks.write().await;
            if let Some(block) = lock.iter_mut().find(|x| x.id == block_data.block_id) {
                block.players -= 1;
            }
        }
    }
    Ok(())
}
