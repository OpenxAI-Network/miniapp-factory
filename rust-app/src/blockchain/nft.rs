use alloy::{primitives::Address, providers::Provider, sol};
use futures_util::StreamExt;

use crate::{
    database::{Database, projects::DatabaseProject},
    utils::env::nft,
};

sol! {
    #[sol(rpc)]
    contract MiniApp {
        event Transfer(address indexed from, address indexed to, uint256 indexed tokenId);

        function mint(address account, uint256 tokenId, string calldata name) external;
    }
}

pub async fn event_listeners<P: Provider>(provider: P, database: Database) {
    let nft = MiniApp::new(nft(), provider);
    let transfer_stream = nft
        .Transfer_filter()
        .subscribe()
        .await
        .unwrap_or_else(|e| panic!("Could not subscribe to mini app transfer event: {e}"))
        .into_stream();

    transfer_stream
        .for_each(async |event| match event {
            Ok((event, _log)) => {
                let from = event.from.to_string();
                let to = event.to.to_string();
                let token_id: i32 = match event.tokenId.try_into() {
                    Ok(token_id) => token_id,
                    Err(e) => {
                        log::error!(
                            "Token id {token_id} could not be converted into i32: {e}",
                            token_id = event.tokenId
                        );
                        return;
                    }
                };

                log::info!("Mini app {token_id} just got transferred from {from} to {to}");
                if Address::parse_checksummed(from, None)
                    .is_ok_and(|address| address == Address::ZERO)
                {
                    // Freshly minted server, database already up to date
                } else {
                    let mut project = match DatabaseProject::get_by_id(&database, token_id).await {
                        Ok(tokenized_server) => match tokenized_server {
                            Some(tokenized_server) => tokenized_server,
                            None => {
                                log::error!("TRANSFER OF NON-EXISTENT PROJECT {token_id}");
                                return;
                            }
                        },
                        Err(e) => {
                            log::error!("FETCHING TRANSFERRED PROJECT {token_id}: {e}");
                            return;
                        }
                    };

                    let owner = to.to_ascii_lowercase().replace("0x", "eth:");
                    if let Err(e) = project.update_owner(&database, owner).await {
                        log::error!("COULD NOT UPDATE PROJECT OWNER {token_id} TO {to}: {e}");
                    };
                }
            }
            Err(e) => {
                log::warn!("Error polling mini app transfer event: {e}")
            }
        })
        .await;
}
