use std::{str::FromStr, time::Duration};

use alloy::{
    primitives::{Address, Uint},
    providers::{Provider, ProviderBuilder},
    signers::local::PrivateKeySigner,
};
use tokio::time;

use crate::{
    blockchain::nft::MiniApp,
    database::{Database, projects::DatabaseProject},
    utils::env::{nft, nftminterkey},
};

pub async fn mint_nfts<P: Provider>(database: Database, provider: P) {
    let mut interval = time::interval(Duration::from_secs(10));

    loop {
        interval.tick().await;

        let mut project = match DatabaseProject::get_next_unminted(&database).await {
            Ok(project) => match project {
                Some(project) => project,
                None => {
                    continue;
                }
            },
            Err(e) => {
                log::error!("Could not get next unfinished deployment: {e}");
                continue;
            }
        };

        let to = match Address::from_str(&project.owner.replace("eth:", "0x")) {
            Ok(address) => address,
            Err(e) => {
                log::error!(
                    "Could not convert project owner {owner} to address: {e}",
                    owner = project.owner
                );
                continue;
            }
        };

        if let Some(tx) = mint_miniapp(&provider, to, project.id, project.name.clone()).await
            && let Err(e) = project.update_nft_mint(&database, Some(tx)).await
        {
            log::error!(
                "Could not update nft mint on {project}: {e}",
                project = project.name
            );
        }
    }
}

pub async fn mint_miniapp<P: Provider>(
    provider: &P,
    to: Address,
    token_id: i32,
    project: String,
) -> Option<String> {
    let signer: PrivateKeySigner = nftminterkey()
        .parse()
        .unwrap_or_else(|e| panic!("Could not parse nftminterkey: {e}"));
    let provider = ProviderBuilder::new()
        .wallet(signer)
        .connect_provider(provider);

    let nft = MiniApp::new(nft(), provider);
    match nft
        .mint(to, Uint::from(token_id), project.clone())
        .send()
        .await
    {
        Ok(tx) => Some(tx.tx_hash().to_string()),
        Err(e) => {
            log::error!(
                "MINT TRANSACTION OF TOKEN {token_id} (PROJECT {project}) TO {to} FAILED: {e}"
            );
            None
        }
    }
}
