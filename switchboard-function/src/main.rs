use std::str::FromStr;

pub use switchboard_solana::get_ixn_discriminator;
pub use switchboard_solana::prelude::*;

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

    // Retrieve provided twitter_user_id BIO
    // Parse BIO to read the last line
    // Last line must match the provided user key
    let validated = true;

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