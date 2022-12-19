use crate::dto::{
    BlockProductionStats, ClusterStats, CommissionRecord, DCConcentrationStats, UptimeRecord,
    ValidatorEpochStats, ValidatorRecord, VersionRecord, WarningRecord,
};
use rust_decimal::prelude::*;
use std::collections::HashMap;
use tokio_postgres::{types::ToSql, Client};

pub struct InsertQueryCombiner<'a> {
    pub insertions: u64,
    statement: String,
    params: Vec<&'a (dyn ToSql + Sync)>,
}

impl<'a> InsertQueryCombiner<'a> {
    pub fn new(table_name: String, columns: String) -> Self {
        Self {
            insertions: 0,
            statement: format!("INSERT INTO {} ({}) VALUES", table_name, columns).to_string(),
            params: vec![],
        }
    }

    pub fn add(&mut self, values: &mut Vec<&'a (dyn ToSql + Sync)>) {
        let separator = if self.insertions == 0 { " " } else { "," };
        let mut query_end = "(".to_string();
        for i in 0..values.len() {
            if i > 0 {
                query_end.push_str(",");
            }
            query_end.push_str(&format!("${}", i + 1 + self.params.len()));
        }
        query_end.push_str(")");

        self.params.append(values);
        self.statement
            .push_str(&format!("{}{}", separator, query_end));
        self.insertions += 1;
    }

    pub async fn execute(&self, client: &mut Client) -> anyhow::Result<Option<u64>> {
        if self.insertions == 0 {
            return Ok(None);
        }

        // println!("{}", self.statement);
        // println!("{:?}", self.params);

        Ok(Some(client.execute(&self.statement, &self.params).await?))
    }
}

pub struct UpdateQueryCombiner<'a> {
    pub updates: u64,
    statement: String,
    values_names: String,
    where_condition: String,
    params: Vec<&'a (dyn ToSql + Sync)>,
}

impl<'a> UpdateQueryCombiner<'a> {
    pub fn new(
        table_name: String,
        updates: String,
        values_names: String,
        where_condition: String,
    ) -> Self {
        Self {
            updates: 0,
            statement: format!("UPDATE {} SET {} FROM (VALUES", table_name, updates).to_string(),
            values_names,
            where_condition,
            params: vec![],
        }
    }

    pub fn add(&mut self, values: &mut Vec<&'a (dyn ToSql + Sync)>, types: HashMap<usize, String>) {
        let separator = if self.updates == 0 { " " } else { "," };
        let mut query_end = "(".to_string();
        for i in 0..values.len() {
            if i > 0 {
                query_end.push_str(",");
            }
            query_end.push_str(&format!("${}", i + 1 + self.params.len()));
            if let Some(t) = types.get(&i) {
                query_end.push_str(&format!("::{}", t));
            };
        }
        query_end.push_str(")");

        self.params.append(values);
        self.statement
            .push_str(&format!("{}{}", separator, query_end));
        self.updates += 1;
    }

    pub async fn execute(&mut self, client: &mut Client) -> anyhow::Result<Option<u64>> {
        if self.updates == 0 {
            return Ok(None);
        }

        self.statement.push_str(&format!(
            ") AS {} WHERE {}",
            self.values_names, self.where_condition
        ));

        // println!("{}", self.statement);
        // println!("{:?}", self.params);

        Ok(Some(client.execute(&self.statement, &self.params).await?))
    }
}

struct InflationApyCalculator {
    supply: u64,
    duration: u64,
    inflation: f64,
    inflation_taper: f64,
    total_weighted_credits: u128,
}
impl InflationApyCalculator {
    fn estimate_yields(&self, credits: u64, stake: u64, commission: u8) -> (f64, f64) {
        let epochs_per_year = 365.25 * 24f64 * 3600f64 / self.duration as f64;
        let rewards_share = credits as f64 * stake as f64 / self.total_weighted_credits as f64;
        let inflation_change_per_epoch = (1.0 - self.inflation_taper).powf(1.0 / epochs_per_year);
        let generated_rewards =
            self.inflation * self.supply as f64 * rewards_share * self.inflation_taper
                / epochs_per_year
                / (1.0 - inflation_change_per_epoch);
        let staker_rewards = generated_rewards * (1.0 - commission as f64 / 100.0);
        let apr = staker_rewards / stake as f64;
        let apy = (1.0 + apr / epochs_per_year).powf(epochs_per_year - 1.0) - 1.0;

        (apr, apy)
    }
}
async fn get_apy_calculators(
    psql_client: &Client,
) -> anyhow::Result<HashMap<u64, InflationApyCalculator>> {
    let apy_info_rows = psql_client
        .query(
            "SELECT
                    epochs.epoch,
                    (EXTRACT('epoch' FROM end_at) - EXTRACT('epoch' FROM start_at))::INTEGER as duration,
                    supply,
                    inflation,
                    inflation_taper,
                    SUM(validators.credits * validators.activated_stake) total_weighted_credits
                FROM
                epochs
                INNER JOIN validators ON epochs.epoch = validators.epoch
                GROUP BY epochs.epoch",
            &[],
        )
        .await?;

    let mut result: HashMap<_, _> = Default::default();
    for row in apy_info_rows {
        result.insert(
            row.get::<_, Decimal>("epoch").try_into()?,
            InflationApyCalculator {
                supply: row.get::<_, Decimal>("supply").try_into()?,
                duration: row.get::<_, i32>("duration").try_into()?,
                inflation: row.get("inflation"),
                inflation_taper: row.get("inflation_taper"),
                total_weighted_credits: row
                    .get::<_, Decimal>("total_weighted_credits")
                    .try_into()?,
            },
        );
    }

    Ok(result)
}

pub async fn load_uptimes(
    psql_client: &Client,
    epochs: u64,
) -> anyhow::Result<HashMap<String, Vec<UptimeRecord>>> {
    let rows = psql_client
        .query(
            "
            WITH cluster AS (SELECT MAX(epoch) as last_epoch FROM cluster_info)
            SELECT
                identity, status, epoch, start_at, end_at
            FROM uptimes, cluster WHERE epoch > cluster.last_epoch - $1::NUMERIC",
            &[&Decimal::from(epochs)],
        )
        .await?;

    let mut records: HashMap<_, Vec<_>> = Default::default();
    for row in rows {
        let identity: String = row.get("identity");
        let commissions = records
            .entry(identity.clone())
            .or_insert(Default::default());
        commissions.push(UptimeRecord {
            epoch: row.get::<_, Decimal>("epoch").try_into()?,
            status: row.get("status"),
            start_at: row.get("start_at"),
            end_at: row.get("end_at"),
        })
    }

    Ok(records)
}

pub async fn load_versions(
    psql_client: &Client,
    epochs: u64,
) -> anyhow::Result<HashMap<String, Vec<VersionRecord>>> {
    let rows = psql_client
        .query(
            "
            WITH cluster AS (SELECT MAX(epoch) as last_epoch FROM cluster_info)
            SELECT
                identity, version, epoch, created_at
            FROM versions, cluster WHERE epoch > cluster.last_epoch - $1::NUMERIC",
            &[&Decimal::from(epochs)],
        )
        .await?;

    let mut records: HashMap<_, Vec<_>> = Default::default();
    for row in rows {
        let identity: String = row.get("identity");
        let commissions = records
            .entry(identity.clone())
            .or_insert(Default::default());
        commissions.push(VersionRecord {
            epoch: row.get::<_, Decimal>("epoch").try_into()?,
            version: row.get("version"),
            created_at: row.get("created_at"),
        })
    }

    Ok(records)
}

pub async fn load_commissions(
    psql_client: &Client,
    epochs: u64,
) -> anyhow::Result<HashMap<String, Vec<CommissionRecord>>> {
    let rows = psql_client
        .query(
            "
            WITH cluster AS (SELECT MAX(epoch) as last_epoch FROM cluster_info)
            SELECT
                identity, commission, epoch, epoch_slot, created_at
            FROM commissions, cluster
            WHERE epoch > cluster.last_epoch - $1::NUMERIC
            UNION
            SELECT
                identity, commission_effective, epoch, 432000, updated_at
            FROM validators, cluster
            WHERE epoch > cluster.last_epoch - $1::NUMERIC AND commission_effective IS NOT NULL
            ",
            &[&Decimal::from(epochs)],
        )
        .await?;

    let mut records: HashMap<_, Vec<_>> = Default::default();
    for row in rows {
        let identity: String = row.get("identity");
        let commissions = records
            .entry(identity.clone())
            .or_insert(Default::default());
        commissions.push(CommissionRecord {
            epoch: row.get::<_, Decimal>("epoch").try_into()?,
            epoch_slot: row.get::<_, Decimal>("epoch_slot").try_into()?,
            commission: row.get::<_, i32>("commission").try_into()?,
            created_at: row.get("created_at"),
        })
    }

    Ok(records)
}

pub async fn load_warnings(
    psql_client: &Client,
) -> anyhow::Result<HashMap<String, Vec<WarningRecord>>> {
    let rows = psql_client
        .query(
            "SELECT identity, code, message, details, created_at FROM warnings",
            &[],
        )
        .await?;

    let mut records: HashMap<_, Vec<_>> = Default::default();
    for row in rows {
        let identity: String = row.get("identity");
        let warnings = records
            .entry(identity.clone())
            .or_insert(Default::default());
        warnings.push(WarningRecord {
            code: row.get("code"),
            message: row.get("message"),
            details: row.get("details"),
            created_at: row.get("created_at"),
        })
    }

    Ok(records)
}

fn average(numbers: &Vec<f64>) -> Option<f64> {
    if numbers.len() == 0 {
        return None;
    }
    Some(numbers.iter().sum::<f64>() / numbers.len() as f64)
}

pub fn update_validators_with_avgs(validators: &mut HashMap<String, ValidatorRecord>) {
    for (_, record) in validators.iter_mut() {
        record.avg_apy = average(
            &record
                .epoch_stats
                .iter()
                .flat_map(|epoch| epoch.apy)
                .collect(),
        );
        record.avg_uptime_pct = average(
            &record
                .epoch_stats
                .iter()
                .flat_map(|epoch| epoch.uptime_pct)
                .collect(),
        );
    }
}

pub fn update_validators_ranks<T>(
    validators: &mut HashMap<String, ValidatorRecord>,
    field_extractor: fn(&ValidatorEpochStats) -> T,
    rank_updater: fn(&mut ValidatorEpochStats, usize) -> (),
) where
    T: Ord,
{
    let mut stats_by_epoch: HashMap<u64, Vec<(String, T)>> = Default::default();
    for (identity, record) in validators.iter() {
        for validator_epoch_stats in record.epoch_stats.iter() {
            stats_by_epoch
                .entry(validator_epoch_stats.epoch)
                .or_insert(Default::default())
                .push((identity.clone(), field_extractor(validator_epoch_stats)));
        }
    }

    for (epoch, stats) in stats_by_epoch.iter_mut() {
        stats.sort_by(|(_, stat_a), (_, stat_b)| stat_a.cmp(stat_b));
        let mut previous_value: Option<&T> = None;
        let mut same_ranks: usize = 0;
        for (index, (identity, stat)) in stats.iter().enumerate() {
            if let Some(some_previous_value) = previous_value {
                if some_previous_value == stat {
                    same_ranks += 1;
                } else {
                    same_ranks = 0;
                }
            }
            previous_value = Some(stat);

            let validator_epoch_stats = validators
                .get_mut(identity)
                .unwrap()
                .epoch_stats
                .iter_mut()
                .find(|a| a.epoch == *epoch)
                .unwrap();
            rank_updater(validator_epoch_stats, stats.len() - index + same_ranks);
        }
    }
}

pub async fn load_validators(
    psql_client: &Client,
    epochs: u64,
) -> anyhow::Result<HashMap<String, ValidatorRecord>> {
    let apy_calculators = get_apy_calculators(psql_client).await?;
    let warnings = load_warnings(psql_client).await?;
    let concentrations = &load_dc_concentration_stats(psql_client, 1).await?.pop();

    log::info!("Querying validators...");
    let rows = psql_client
        .query(
            "
            WITH
                validators_aggregated AS (SELECT identity, MIN(epoch) first_epoch FROM validators GROUP BY identity),
                cluster AS (SELECT MAX(epoch) as last_epoch FROM cluster_info)
            SELECT
                validators.identity, vote_account, epoch,

                info_name,
                info_url,
                info_keybase,
                node_ip,
                dc_coordinates_lat,
                dc_coordinates_lon,
                dc_continent,
                dc_country_iso,
                dc_country,
                dc_city,
                dc_asn,
                dc_aso,
                CONCAT(dc_continent, '/', dc_country, '/', dc_city) dc_full_city,

                commission_max_observed,
                commission_min_observed,
                commission_advertised,
                commission_effective,
                version,
                mnde_votes,
                activated_stake,
                marinade_stake,
                decentralizer_stake,
                superminority,
                stake_to_become_superminority,
                credits,
                leader_slots,
                blocks_produced,
                skip_rate,
                uptime_pct,
                uptime,
                downtime,

                validators_aggregated.first_epoch AS first_epoch
            FROM validators
                LEFT JOIN cluster ON 1 = 1
                LEFT JOIN validators_aggregated ON validators_aggregated.identity = validators.identity
            WHERE epoch > cluster.last_epoch - $1::NUMERIC
            ORDER BY epoch DESC",
            &[&Decimal::from(epochs)],
        )
        .await?;

    log::info!("Aggregating validator records...");
    let mut records: HashMap<_, _> = Default::default();
    for row in rows {
        let identity: String = row.get("identity");
        let epoch: u64 = row.get::<_, Decimal>("epoch").try_into()?;
        let first_epoch: u64 = row.get::<_, Decimal>("first_epoch").try_into().unwrap();
        let (apr, apy) = if let Some(c) = apy_calculators.get(&epoch) {
            let (apr, apy) = c.estimate_yields(
                row.get::<_, Decimal>("credits").try_into()?,
                row.get::<_, Decimal>("activated_stake").try_into()?,
                row.get::<_, Option<i32>>("commission_effective")
                    .map(|n| n.try_into().unwrap())
                    .unwrap_or(100),
            );
            (Some(apr), Some(apy))
        } else {
            (None, None)
        };

        let dc_full_city = row
            .get::<_, Option<String>>("dc_full_city")
            .unwrap_or("Unknown".into());
        let dc_asn = row
            .get::<_, Option<i32>>("dc_asn")
            .map(|dc_asn| dc_asn.to_string())
            .unwrap_or("Unknown".into());
        let dc_aso = row
            .get::<_, Option<String>>("dc_aso")
            .unwrap_or("Unknown".into());

        let dcc_full_city = concentrations
            .clone()
            .and_then(|c| c.dc_concentration_by_city.get(&dc_full_city).cloned());
        let dcc_asn = concentrations
            .clone()
            .and_then(|c| c.dc_concentration_by_asn.get(&dc_asn).cloned());
        let dcc_aso = concentrations
            .clone()
            .and_then(|c| c.dc_concentration_by_aso.get(&dc_aso).cloned());

        let record = records
            .entry(identity.clone())
            .or_insert_with(|| ValidatorRecord {
                identity: identity.clone(),
                vote_account: row.get("vote_account"),
                info_name: row.get("info_name"),
                info_url: row.get("info_url"),
                info_keybase: row.get("info_keybase"),
                node_ip: row.get("node_ip"),
                dc_coordinates_lat: row.get("dc_coordinates_lat"),
                dc_coordinates_lon: row.get("dc_coordinates_lon"),
                dc_continent: row.get("dc_continent"),
                dc_country_iso: row.get("dc_country_iso"),
                dc_country: row.get("dc_country"),
                dc_city: row.get("dc_city"),
                dc_full_city: row.get("dc_full_city"),
                dc_asn: row.get("dc_asn"),
                dc_aso: row.get("dc_aso"),
                dcc_full_city,
                dcc_asn,
                dcc_aso,
                commission_max_observed: row
                    .get::<_, Option<i32>>("commission_max_observed")
                    .map(|n| n.try_into().unwrap()),
                commission_min_observed: row
                    .get::<_, Option<i32>>("commission_min_observed")
                    .map(|n| n.try_into().unwrap()),
                commission_advertised: row
                    .get::<_, Option<i32>>("commission_advertised")
                    .map(|n| n.try_into().unwrap()),
                commission_effective: row
                    .get::<_, Option<i32>>("commission_effective")
                    .map(|n| n.try_into().unwrap()),
                commission_aggregated: None,
                version: row.get("version"),
                mnde_votes: row
                    .get::<_, Option<Decimal>>("mnde_votes")
                    .map(|n| n.try_into().unwrap()),
                activated_stake: row.get::<_, Decimal>("activated_stake").try_into().unwrap(),
                marinade_stake: row.get::<_, Decimal>("marinade_stake").try_into().unwrap(),
                decentralizer_stake: row
                    .get::<_, Decimal>("decentralizer_stake")
                    .try_into()
                    .unwrap(),
                superminority: row.get("superminority"),
                credits: row.get::<_, Decimal>("credits").try_into().unwrap(),
                marinade_score: 0,

                epoch_stats: Default::default(),

                warnings: warnings.get(&identity).cloned().unwrap_or(vec![]),

                epochs_count: epoch - first_epoch + 1,

                avg_uptime_pct: None,
                avg_apy: None,
            });
        record.epoch_stats.push(ValidatorEpochStats {
            epoch,
            commission_max_observed: row
                .get::<_, Option<i32>>("commission_max_observed")
                .map(|n| n.try_into().unwrap()),
            commission_min_observed: row
                .get::<_, Option<i32>>("commission_min_observed")
                .map(|n| n.try_into().unwrap()),
            commission_advertised: row
                .get::<_, Option<i32>>("commission_advertised")
                .map(|n| n.try_into().unwrap()),
            commission_effective: row
                .get::<_, Option<i32>>("commission_effective")
                .map(|n| n.try_into().unwrap()),
            version: row.get("version"),
            mnde_votes: row
                .get::<_, Option<Decimal>>("mnde_votes")
                .map(|n| n.try_into().unwrap()),
            activated_stake: row.get::<_, Decimal>("activated_stake").try_into()?,
            marinade_stake: row.get::<_, Decimal>("marinade_stake").try_into()?,
            decentralizer_stake: row.get::<_, Decimal>("decentralizer_stake").try_into()?,
            superminority: row.get("superminority"),
            stake_to_become_superminority: row
                .get::<_, Decimal>("stake_to_become_superminority")
                .try_into()?,
            credits: row.get::<_, Decimal>("credits").try_into()?,
            leader_slots: row.get::<_, Decimal>("leader_slots").try_into()?,
            blocks_produced: row.get::<_, Decimal>("blocks_produced").try_into()?,
            skip_rate: row.get("skip_rate"),
            uptime_pct: row.get("uptime_pct"),
            uptime: row
                .get::<_, Option<Decimal>>("uptime")
                .map(|n| n.try_into().unwrap()),
            downtime: row
                .get::<_, Option<Decimal>>("downtime")
                .map(|n| n.try_into().unwrap()),
            apr,
            apy,
            marinade_score: 0,
            rank_apy: None,
            rank_marinade_score: None,
            rank_activated_stake: None,
        });
    }

    log::info!("Updating averages...");
    update_validators_with_avgs(&mut records);
    log::info!("Updating ranks...");
    update_validators_ranks(
        &mut records,
        |a: &ValidatorEpochStats| a.activated_stake,
        |a: &mut ValidatorEpochStats, rank: usize| a.rank_activated_stake = Some(rank),
    );
    update_validators_ranks(
        &mut records,
        |a: &ValidatorEpochStats| a.marinade_score,
        |a: &mut ValidatorEpochStats, rank: usize| a.rank_marinade_score = Some(rank),
    );
    update_validators_ranks(
        &mut records,
        |a: &ValidatorEpochStats| (a.apy.unwrap_or(0.0) * 1000.0) as u64,
        |a: &mut ValidatorEpochStats, rank: usize| a.rank_marinade_score = Some(rank),
    );
    log::info!("Records prepared...");
    Ok(records)
}

pub async fn get_last_epoch(psql_client: &Client) -> anyhow::Result<Option<u64>> {
    let row = psql_client
        .query_opt("SELECT MAX(epoch) as last_epoch FROM validators", &[])
        .await?;

    Ok(row.map(|row| row.get::<_, Decimal>("last_epoch").try_into().unwrap()))
}

pub async fn load_dc_concentration_stats(
    psql_client: &Client,
    epochs: u64,
) -> anyhow::Result<Vec<DCConcentrationStats>> {
    let last_epoch = match get_last_epoch(psql_client).await? {
        Some(last_epoch) => last_epoch,
        _ => return Ok(Default::default()),
    };
    let first_epoch = last_epoch - epochs.min(last_epoch) + 1;

    let mut stats: Vec<_> = Default::default();

    let map_stake_to_concentration =
        |stake: &HashMap<String, u64>, total_stake: u64| -> HashMap<_, _> {
            stake
                .iter()
                .map(|(key, stake)| (key.clone(), *stake as f64 / total_stake as f64))
                .collect()
        };

    for epoch in first_epoch..=last_epoch {
        let mut dc_stake_by_aso: HashMap<_, _> = Default::default();
        let mut dc_stake_by_asn: HashMap<_, _> = Default::default();
        let mut dc_stake_by_city: HashMap<_, _> = Default::default();
        let mut total_active_stake = 0;

        let rows = psql_client
            .query(
                "SELECT
                    activated_stake,
                    dc_aso,
                    dc_asn,
                    CONCAT(dc_continent, '/', dc_country, '/', dc_city) dc_full_city
                FROM validators WHERE epoch = $1",
                &[&Decimal::from(epoch)],
            )
            .await?;

        for row in rows.iter() {
            let activated_stake: u64 = row.get::<_, Decimal>("activated_stake").try_into()?;
            let dc_aso = row
                .get::<_, Option<String>>("dc_aso")
                .unwrap_or("Unknown".to_string());
            let dc_asn: String = row
                .get::<_, Option<i32>>("dc_asn")
                .map_or("Unknown".to_string(), |dc_asn| dc_asn.to_string());
            let dc_city: String = row
                .get::<_, Option<String>>("dc_full_city")
                .unwrap_or("Unknown".to_string());

            total_active_stake += activated_stake;
            *(dc_stake_by_aso.entry(dc_aso).or_insert(Default::default())) += activated_stake;
            *(dc_stake_by_asn.entry(dc_asn).or_insert(Default::default())) += activated_stake;
            *(dc_stake_by_city
                .entry(dc_city)
                .or_insert(Default::default())) += activated_stake;
        }

        stats.push(DCConcentrationStats {
            epoch: Default::default(),
            total_activated_stake: Default::default(),
            dc_concentration_by_aso: map_stake_to_concentration(
                &dc_stake_by_aso,
                total_active_stake,
            ),
            dc_concentration_by_asn: map_stake_to_concentration(
                &dc_stake_by_asn,
                total_active_stake,
            ),
            dc_stake_by_asn,
            dc_stake_by_aso,
            dc_concentration_by_city: map_stake_to_concentration(
                &dc_stake_by_city,
                total_active_stake,
            ),
            dc_stake_by_city,
        })
    }

    Ok(stats)
}

pub async fn load_block_production_stats(
    psql_client: &Client,
    epochs: u64,
) -> anyhow::Result<Vec<BlockProductionStats>> {
    let last_epoch = match get_last_epoch(psql_client).await? {
        Some(last_epoch) => last_epoch,
        _ => return Ok(Default::default()),
    };
    let first_epoch = last_epoch - epochs.min(last_epoch) + 1;

    let mut stats: Vec<_> = Default::default();

    let rows = psql_client
            .query(
                "SELECT
	                epoch,
                    COALESCE(SUM(blocks_produced), 0) blocks_produced,
                    COALESCE(SUM(leader_slots), 0) leader_slots,
                    1 - COALESCE(SUM(blocks_produced), 0)  / coalesce(SUM(leader_slots), 1) avg_skip_rate
                FROM validators
                WHERE epoch > $1
                GROUP BY epoch ORDER BY epoch",
                &[&Decimal::from(first_epoch)],
            )
            .await?;

    for row in rows {
        stats.push(BlockProductionStats {
            epoch: row.get::<_, Decimal>("epoch").try_into()?,
            blocks_produced: row.get::<_, Decimal>("blocks_produced").try_into()?,
            leader_slots: row.get::<_, Decimal>("leader_slots").try_into()?,
            avg_skip_rate: row.get("avg_skip_rate"),
        })
    }

    Ok(stats)
}

pub async fn load_cluster_info(psql_client: &Client, epochs: u64) -> anyhow::Result<ClusterStats> {
    Ok(ClusterStats {
        block_production_stats: load_block_production_stats(psql_client, epochs).await?,
        dc_concentration_stats: load_dc_concentration_stats(psql_client, epochs).await?,
    })
}
