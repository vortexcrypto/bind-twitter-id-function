use std::str::FromStr;

use regex::Regex;
pub use switchboard_solana::get_ixn_discriminator;
pub use switchboard_solana::prelude::*;
use twitter_v2::authorization::BearerToken;
use twitter_v2::query::UserField;
use twitter_v2::TwitterApi;

mod params;
pub use params::*;

#[tokio::main(worker_threads = 12)]
async fn main() {
    // First, initialize the runner instance with a freshly generated Gramine keypair
    let runner = FunctionRunner::new_from_cluster(Cluster::Devnet, None).unwrap();

    // parse and validate user provided request params
    let params = ContainerParams::decode(
        &runner
            .function_request_data
            .as_ref()
            .unwrap()
            .container_params,
    )
    .unwrap();

    // Retrieve provided twitter_username BIO
    // Parse BIO and extract wallet address
    // wallet must match provided wallet
    let auth = BearerToken::new("APP_BEARER_TOKEN");
    let twitter_api = TwitterApi::new(auth);

    let maybe_wallet =
        get_wallet_from_user_bio(&twitter_api, params.twitter_username.as_str()).await;

    let validated: bool = maybe_wallet.is_ok() && maybe_wallet.unwrap().eq(&params.wallet);

    // IXN DATA:
    // LEN: 12 bytes
    // [0-8]: Anchor Ixn Discriminator
    // [9-10]: Outcome (bool)
    let mut ixn_data = get_ixn_discriminator("bind_twitter_id_settle").to_vec();
    ixn_data.append(&mut vec![validated as u8]);

    // ACCOUNTS:
    // 1. Enclave Signer (signer): our Gramine generated keypair
    // 2. User: our user who made the request
    // 3. Realm
    // 4. User Account PDA
    // 5.
    // 6. Switchboard Function
    // 7. Switchboard Function Request
    let settle_ixn = Instruction {
        program_id: params.program_id,
        data: ixn_data,
        accounts: vec![
            AccountMeta::new_readonly(runner.signer, true),
            AccountMeta::new_readonly(params.user, false),
            AccountMeta::new(params.realm_pda, false),
            AccountMeta::new(params.user_account_pda, false),
            AccountMeta::new(params.user_account_pda, false), // TODO
            AccountMeta::new_readonly(runner.function, false),
            AccountMeta::new_readonly(runner.function_request_key.unwrap(), false),
        ],
    };

    // Then, write your own Rust logic and build a Vec of instructions.
    // Should  be under 700 bytes after serialization
    let ixs: Vec<solana_program::instruction::Instruction> = vec![settle_ixn];

    // Finally, emit the signed quote and partially signed transaction to the functionRunner oracle
    // The functionRunner oracle will use the last outputted word to stdout as the serialized result. This is what gets executed on-chain.
    runner.emit(ixs).await.unwrap();
}

pub async fn get_user_by_username(
    twitter_api: &TwitterApi<BearerToken>,
    username: &str,
    user_fields: Vec<twitter_v2::query::UserField>,
) -> std::result::Result<twitter_v2::data::User, SbError> {
    if let Some(user) = twitter_api
        .get_user_by_username(username)
        .user_fields(user_fields.into_iter())
        .send()
        .await
        .map_err(|e| {
            println!("err getting user: {:?}", e);

            SbError::CustomMessage("err getting user".to_string())
        })?
        .data()
    {
        return Ok(user.clone());
    }

    Err(SbError::CustomMessage("user not found".to_string()))
}

pub fn extract_solana_wallet_address_from_string(
    str: &str,
) -> std::result::Result<String, SbError> {
    let re = Regex::new(r"[1-9A-HJ-NP-Za-km-z]{32,44}").unwrap();

    let matches: Vec<_> = re.captures_iter(str).collect();

    if matches.len() != 1 {
        return Err(SbError::CustomMessage("wallet not found".to_string()));
    }

    let wallet = String::from_str(matches.first().unwrap().get(0).unwrap().as_str()).unwrap();

    Ok(wallet)
}

pub async fn get_wallet_from_user_bio(
    twitter_api: &TwitterApi<BearerToken>,
    username: &str,
) -> std::result::Result<Pubkey, SbError> {
    let user = get_user_by_username(twitter_api, username, vec![UserField::Description]).await?;

    println!("user: {:?}", user);

    if let Some(description) = user.description {
        let wallet = extract_solana_wallet_address_from_string(description.as_str())?;

        let maybe_pubkey = Pubkey::from_str(wallet.as_str());

        if maybe_pubkey.is_err() {
            return Err(SbError::CustomMessage(
                "wallet is not a valid pubkey".to_string(),
            ));
        }

        return Ok(maybe_pubkey.unwrap());
    }

    Err(SbError::CustomMessage("wallet not found".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_extract_wallet() {
        let wallet = extract_solana_wallet_address_from_string(
            "hello how are you? F4hzp6TKSUJ5xvXzcQwvBH3XTmYJ16HuTX9t4gNQabib",
        )
        .unwrap();

        assert_eq!(wallet, "F4hzp6TKSUJ5xvXzcQwvBH3XTmYJ16HuTX9t4gNQabib");
    }

    #[test]
    pub fn test_extract_multiple_wallet() {
        assert!(extract_solana_wallet_address_from_string(
            "hello CkbxaunPif9H3Zq24nyY81pKUe64GRteciPL5qXLUdzC how are you? F4hzp6TKSUJ5xvXzcQwvBH3XTmYJ16HuTX9t4gNQabib",
        ).is_err());
    }

    #[tokio::test]
    pub async fn test_extract_wallet_from_twitter_bio() {
        // Use dev@vortexcrypto.io dev account
        let auth = BearerToken::new("APP_BEARER_TOKEN");
        let twitter_api = TwitterApi::new(auth);

        let wallet = get_wallet_from_user_bio(&twitter_api, "orelsanpls")
            .await
            .unwrap();

        assert_eq!(
            wallet,
            Pubkey::from_str("WabxR2gcdMgovS6Uo5JD4Cv9me7uExRyaH4QDKrp64b").unwrap()
        );
    }
}
