//! Stress tests for liveness and consistency properties.

use std::time::Duration;

use anyhow::{anyhow, Result};
use common::*;
use log::info;
use operational_transform::OperationSeq;
use rustpad_server::server;
use serde_json::{json, Value};
use tokio::time::Instant;

pub mod common;

#[tokio::test]
async fn test_lost_wakeups() -> Result<()> {
    pretty_env_logger::try_init().ok();
    let filter = server();

    expect_text(&filter, "stress", "").await;

    let mut client = connect(&filter, "stress").await?;
    let msg = client.recv().await?;
    assert_eq!(msg, json!({ "Identity": 0 }));

    let mut client2 = connect(&filter, "stress").await?;
    let msg = client2.recv().await?;
    assert_eq!(msg, json!({ "Identity": 1 }));

    let mut revision = 0;
    for i in 0..100 {
        let num_edits = i % 5 + 1;
        for _ in 0..num_edits {
            let mut operation = OperationSeq::default();
            operation.retain(revision);
            operation.insert("a");
            let msg = json!({
                "Edit": {
                    "revision": revision,
                    "operation": operation
                }
            });
            client.send(&msg).await;
            revision += 1;
        }

        let start = Instant::now();

        let num_ops = |msg: &Value| -> Option<usize> {
            Some(msg.get("History")?.get("operations")?.as_array()?.len())
        };

        let mut total = 0;
        while total < num_edits {
            let msg = client.recv().await?;
            total += num_ops(&msg).ok_or(anyhow!("missing json key"))?;
        }

        let mut total2 = 0;
        while total2 < num_edits {
            let msg = client2.recv().await?;
            total2 += num_ops(&msg).ok_or(anyhow!("missing json key"))?;
        }

        info!("took {} ms", start.elapsed().as_millis());
        assert!(start.elapsed() <= Duration::from_millis(200));
    }

    expect_text(&filter, "stress", &"a".repeat(revision as usize)).await;

    Ok(())
}
