use std::sync::Arc;
use rsnano_node::node::Node;
use rsnano_rpc_messages::{WorkGenerateArgs, WorkGenerateDto};
use rsnano_core::WorkVersion;

pub async fn work_generate(node: Arc<Node>, enable_control: bool, args: WorkGenerateArgs) -> Result<WorkGenerateDto, String> {
    let work_version = args.version.unwrap_or(WorkVersion::Work1);
    let difficulty = args.difficulty.unwrap(); //_or_else(|| node.default_difficulty(work_version));
    
    // Validate difficulty
    //if difficulty > node.max_work_generate_difficulty(work_version) || 
       //difficulty < node.network_params.work.threshold_entry(work_version, BlockType::State) {
        //return Err("Difficulty out of valid range".to_string());
    //}

    // Handle block if provided
    if let Some(block) = args.block {
        if args.version.is_some() && work_version != WorkVersion::Work1 {
            return Err("Block work version mismatch".to_string());
        }
        // Recalculate difficulty if not provided
        if args.difficulty.is_none() && args.multiplier.is_none() {
            //difficulty = node.difficulty_ledger(&block);
        }
        if node.network_params.work.difficulty(work_version, &args.hash.into(), 0) >= difficulty {
            return Err("Block work is already sufficient".to_string());
        }
    }

    let use_peers = args.use_peers.unwrap_or(false);
    //let account = args.account.unwrap();
    //args.account.or_else(|| {
        // Fetch account from block if not given
        //node.ledger.block_account(&args.hash)
    //});

    let work_result = if !use_peers {
        //if node.local_work_generation_enabled() {
            node.distributed_work.generate_in_local_work_pool(args.hash.into(), difficulty).await
        //} else {
            //return Err("Local work generation is disabled".to_string());
        //}
    } else {
        if node.distributed_work.work_generation_enabled() {
            node.distributed_work.make(args.hash.into(), difficulty, args.account).await

        } else {
            return Err("Work generation is disabled".to_string());
        }
    };

    let result_difficulty = node.network_params.work.difficulty(work_version, &args.hash.into(), work_result.unwrap());
    //let result_multiplier = node.difficulty_to_multiplier(result_difficulty, node.default_difficulty(work_version));

    Ok(WorkGenerateDto::new(
        work_result.unwrap().into(),
        result_difficulty,
        Some(0.),
        args.hash,
    ))
}