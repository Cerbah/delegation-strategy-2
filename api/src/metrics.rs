use lazy_static::lazy_static;
use log::error;
use prometheus::{
    register_int_counter, register_int_gauge_vec, Encoder, IntCounter, IntGaugeVec, TextEncoder,
};
use warp::Filter;

lazy_static! {
    pub static ref REQUEST_CLUSTER_STATS: IntCounter = register_int_counter!(
        "ds_request_count_cluster_stats",
        "How many times /cluster-stats endpoint was requested"
    )
    .unwrap();
    pub static ref REQUEST_UNSTAKE_HINTS: IntCounter = register_int_counter!(
        "ds_request_count_unstake_hints",
        "How many times /unstake-hints endpoint was requested"
    )
    .unwrap();
    pub static ref REQUEST_COUNT_VALIDATORS: IntCounter = register_int_counter!(
        "ds_request_count_validators",
        "How many times /validators endpoint was requested"
    )
    .unwrap();
    pub static ref REQUEST_COUNT_VALIDATOR_SCORE_BREAKDOWN: IntCounter = register_int_counter!(
        "ds_request_count_validator_score_breakdown",
        "How many times /validators/score-breakdown endpoint was requested"
    )
    .unwrap();
    pub static ref REQUEST_COUNT_VALIDATOR_SCORES: IntCounter = register_int_counter!(
        "ds_request_count_validator_scores",
        "How many times /validators/scores endpoint was requested"
    )
    .unwrap();
    pub static ref REQUEST_COUNT_REPORT_STAKING: IntCounter = register_int_counter!(
        "ds_request_count_planned_stakes",
        "How many times reports/staking endpoint was requested"
    )
    .unwrap();
    pub static ref REQUEST_COUNT_VALIDATORS_FLAT: IntCounter = register_int_counter!(
        "ds_request_count_validators_flat",
        "How many times /validators/flat endpoint was requested"
    )
    .unwrap();
    pub static ref REQUEST_COUNT_COMMISSIONS: IntCounter = register_int_counter!(
        "ds_request_count_commissions",
        "How many times /commissions endpoint was requested"
    )
    .unwrap();
    pub static ref REQUEST_COUNT_VERSIONS: IntCounter = register_int_counter!(
        "ds_request_count_versions",
        "How many times /versions endpoint was requested"
    )
    .unwrap();
    pub static ref REQUEST_COUNT_UPTIMES: IntCounter = register_int_counter!(
        "ds_request_count_uptimes",
        "How many times /uptimes endpoint was requested"
    )
    .unwrap();
    pub static ref REQUEST_ADMIN_SCORE_UPLOAD: IntCounter = register_int_counter!(
        "ds_request_count_admin_score_upload",
        "How many times /admin/scores endpoint was requested"
    )
    .unwrap();
    pub static ref JOB_COUNT_SCHEDULED: IntCounter =
        register_int_counter!("ds_job_count_scheduled", "How many jobs were scheduled").unwrap();
    pub static ref JOB_COUNT_SUCCESS: IntCounter =
        register_int_counter!("ds_job_count_success", "How many jobs succeded").unwrap();
    pub static ref JOB_COUNT_ERROR: IntCounter =
        register_int_counter!("ds_job_count_error", "How many jobs failed").unwrap();
    pub static ref JOB_DURATION: IntGaugeVec =
        register_int_gauge_vec!("ds_job_duration", "Workflow jobs duration", &["workflow"])
            .unwrap();
}

fn collect_metrics() -> String {
    let mut buffer = Vec::new();
    let encoder = TextEncoder::new();

    encoder.encode(&prometheus::gather(), &mut buffer).unwrap();

    String::from_utf8(buffer.clone()).unwrap()
}

pub fn spawn_server() {
    tokio::spawn(async move {
        let route_metrics = warp::path!("metrics")
            .and(warp::path::end())
            .and(warp::get())
            .map(|| collect_metrics());
        warp::serve(route_metrics).run(([0, 0, 0, 0], 9000)).await;
        error!("Metrics server is dead.");
        std::process::exit(1);
    });
}
