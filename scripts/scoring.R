library(dotenv)
library(semver)
library(data.table)

normalize <- function(x, na.rm = TRUE) {
  return ((x - min(x, na.rm = TRUE)) / (max(x, na.rm = TRUE) - min(x, na.rm = TRUE)))
}

if (length(commandArgs(trailingOnly=TRUE)) > 0) {
  args <- commandArgs(trailingOnly=TRUE)
}
file_out_scores <- args[1]
file_out_stakes <- args[2]
file_params <- args[3]
file_blacklist <- args[4]
file_validators <- args[5]
file_msol_votes <- args[6]

t(data.frame(
  file_out_scores,
  file_out_stakes,
  file_params,
  file_blacklist,
  file_validators,
  file_msol_votes
))

msol_votes <- read.csv(file_msol_votes)
validators <- read.csv(file_validators)
blacklist <- read.csv(file_blacklist)
load_dot_env(file = file_params)

TOTAL_STAKE=as.numeric(Sys.getenv("TOTAL_STAKE"))

MARINADE_VALIDATORS_COUNT <- as.numeric(Sys.getenv("MARINADE_VALIDATORS_COUNT"))

WEIGHT_ADJUSTED_CREDITS <- as.numeric(Sys.getenv("WEIGHT_ADJUSTED_CREDITS"))
WEIGHT_GRACE_SKIP_RATE <- as.numeric(Sys.getenv("WEIGHT_GRACE_SKIP_RATE"))
WEIGHT_DC_CONCENTRATION <- as.numeric(Sys.getenv("WEIGHT_DC_CONCENTRATION"))

ELIGIBILITY_ALGO_STAKE_MAX_COMMISSION <- as.numeric(Sys.getenv("ELIGIBILITY_ALGO_STAKE_MAX_COMMISSION"))
ELIGIBILITY_ALGO_STAKE_MIN_STAKE <- as.numeric(Sys.getenv("ELIGIBILITY_ALGO_STAKE_MIN_STAKE"))

ELIGIBILITY_MNDE_STAKE_MAX_COMMISSION <- as.numeric(Sys.getenv("ELIGIBILITY_MNDE_STAKE_MAX_COMMISSION"))
ELIGIBILITY_MNDE_STAKE_MIN_STAKE <- as.numeric(Sys.getenv("ELIGIBILITY_MNDE_STAKE_MIN_STAKE"))
ELIGIBILITY_MNDE_SCORE_THRESHOLD_MULTIPLIER <- as.numeric(Sys.getenv("ELIGIBILITY_MNDE_SCORE_THRESHOLD_MULTIPLIER"))

ELIGIBILITY_MSOL_STAKE_MAX_COMMISSION <- as.numeric(Sys.getenv("ELIGIBILITY_MSOL_STAKE_MAX_COMMISSION"))
ELIGIBILITY_MSOL_STAKE_MIN_STAKE <- as.numeric(Sys.getenv("ELIGIBILITY_MSOL_STAKE_MIN_STAKE"))
ELIGIBILITY_MSOL_SCORE_THRESHOLD_MULTIPLIER <- as.numeric(Sys.getenv("ELIGIBILITY_MSOL_SCORE_THRESHOLD_MULTIPLIER"))

ELIGIBILITY_MIN_VERSION <- Sys.getenv("ELIGIBILITY_MIN_VERSION")

MNDE_VALIDATOR_CAP <- as.numeric(Sys.getenv("MNDE_VALIDATOR_CAP"))

STAKE_CONTROL_MNDE <- as.numeric(Sys.getenv("STAKE_CONTROL_MNDE"))
STAKE_CONTROL_MSOL <- as.numeric(Sys.getenv("STAKE_CONTROL_MSOL"))
STAKE_CONTROL_ALGO <- 1 - STAKE_CONTROL_MNDE - STAKE_CONTROL_MSOL

# Perform min-max normalization of algo staking formula's components
validators$normalized_dc_concentration <- normalize(1 - validators$avg_dc_concentration)
validators$normalized_grace_skip_rate <- normalize(1 - validators$avg_grace_skip_rate)
validators$normalized_adjusted_credits <- normalize(validators$avg_adjusted_credits)
validators$rank_dc_concentration <- rank(-validators$normalized_dc_concentration, ties.method="min")
validators$rank_grace_skip_rate <- rank(-validators$normalized_grace_skip_rate, ties.method="min")
validators$rank_adjusted_credits <- rank(-validators$normalized_adjusted_credits, ties.method="min")

# Apply the algo staking formula on all validators
validators$score <- (0
                     + validators$normalized_dc_concentration * WEIGHT_DC_CONCENTRATION
                     + validators$normalized_grace_skip_rate * WEIGHT_GRACE_SKIP_RATE
                     + validators$normalized_adjusted_credits * WEIGHT_ADJUSTED_CREDITS
) / (WEIGHT_ADJUSTED_CREDITS + WEIGHT_GRACE_SKIP_RATE + WEIGHT_DC_CONCENTRATION)

# Apply blacklist
validators$blacklisted <- 0
for (i in 1:nrow(validators)) {
  blacklist_reasons <- blacklist[blacklist$vote_account == validators[i, "vote_account"],]
  if (nrow(blacklist_reasons) > 0) {
    for (j in 1:nrow(blacklist_reasons)) {
        validators[i, "blacklisted"] <- 1
        validators[i, "ui_hints"][[1]] <- list(c(validators[i, "ui_hints"][[1]], blacklist_reasons[j, "code"]))
    }
  }
}

# Apply algo staking eligibility criteria
validators$eligible_stake_algo <- 1 - validators$blacklisted
validators$eligible_stake_algo[validators$max_commission > ELIGIBILITY_ALGO_STAKE_MAX_COMMISSION] <- 0
validators$eligible_stake_algo[validators$minimum_stake < ELIGIBILITY_ALGO_STAKE_MIN_STAKE] <- 0
validators$eligible_stake_algo[parse_version(validators$version) < ELIGIBILITY_MIN_VERSION] <- 0

validators$eligible_stake_msol <- validators$eligible_stake_algo

for (i in 1:nrow(validators)) {
  if (validators[i, "max_commission"] > ELIGIBILITY_ALGO_STAKE_MAX_COMMISSION) {
    validators[i, "ui_hints"][[1]] <- list(c(validators[i, "ui_hints"][[1]], "NOT_ELIGIBLE_ALGO_STAKE_MAX_COMMISSION_OVER_10"))
  }
  if (validators[i, "minimum_stake"] < ELIGIBILITY_ALGO_STAKE_MIN_STAKE) {
    validators[i, "ui_hints"][[1]] <- list(c(validators[i, "ui_hints"][[1]], "NOT_ELIGIBLE_ALGO_STAKE_MIN_STAKE_BELOW_1000"))
  }
  if (parse_version(validators[i, "version"]) < ELIGIBILITY_MIN_VERSION) {
    validators[i, "ui_hints"][[1]] <- list(c(validators[i, "ui_hints"][[1]], "NOT_ELIGIBLE_VERSION_TOO_LOW"))
  }
}

# Sort validators to find the eligible validators with the best score
validators$rank <- rank(-validators$score, ties.method="min")
validators <- validators[order(validators$rank),]
validators_algo_set <- head(validators[validators$eligible_stake_algo == 1,], MARINADE_VALIDATORS_COUNT)
min_score_in_algo_set <- min(validators_algo_set$score)

# Mark validator who should receive algo stake
validators$in_algo_stake_set <- 0
validators$in_algo_stake_set[validators$score >= min_score_in_algo_set] <- 1
validators$in_algo_stake_set[validators$eligible_stake_algo == 0] <- 0

# Mark msol votes for each validator
validators$msol_votes <- 0
if (nrow(msol_votes) > 0) {
  for(i in 1:nrow(msol_votes)) {
    validators[validators$vote_account == msol_votes[i, "vote_account"], ]$msol_votes <- msol_votes[i, "msol_votes"]
  }
}

# Apply msol staking eligibility criteria
validators$eligible_stake_msol <- 1 - validators$blacklisted
validators$eligible_stake_msol[validators$max_commission > ELIGIBILITY_MSOL_STAKE_MAX_COMMISSION] <- 0
validators$eligible_stake_msol[validators$minimum_stake < ELIGIBILITY_MSOL_STAKE_MIN_STAKE] <- 0
validators$eligible_stake_msol[validators$score < min_score_in_algo_set * ELIGIBILITY_MSOL_SCORE_THRESHOLD_MULTIPLIER] <- 0
validators$eligible_stake_msol[parse_version(validators$version) < ELIGIBILITY_MIN_VERSION] <- 0 # UI hint provided earlier

for (i in 1:nrow(validators)) {
  if (validators[i, "max_commission"] > ELIGIBILITY_MSOL_STAKE_MAX_COMMISSION) {
    validators[i, "ui_hints"][[1]] <- list(c(validators[i, "ui_hints"][[1]], "NOT_ELIGIBLE_MSOL_STAKE_MAX_COMMISSION_OVER_10"))
  }
  if (validators[i, "minimum_stake"] < ELIGIBILITY_MSOL_STAKE_MIN_STAKE) {
    validators[i, "ui_hints"][[1]] <- list(c(validators[i, "ui_hints"][[1]], "NOT_ELIGIBLE_MSOL_STAKE_MIN_STAKE_BELOW_100"))
  }
  if (validators[i, "score"] < min_score_in_algo_set * ELIGIBILITY_MSOL_SCORE_THRESHOLD_MULTIPLIER) {
    validators[i, "ui_hints"][[1]] <- list(c(validators[i, "ui_hints"][[1]], "NOT_ELIGIBLE_MSOL_STAKE_SCORE_TOO_LOW"))
  }
}

# Apply eligibility on votes to get effective votes
msol_valid_votes <- round(validators$msol_votes * validators$eligible_stake_msol)
msol_valid_votes_total <- sum(msol_valid_votes)

validators$msol_power <- 0
if (msol_valid_votes_total > 0) {
  validators$msol_power <- msol_valid_votes / msol_valid_votes_total
}

# Apply mnde staking eligibility criteria
validators$eligible_stake_mnde <- 1 - validators$blacklisted
validators$eligible_stake_mnde[validators$max_commission > ELIGIBILITY_MNDE_STAKE_MAX_COMMISSION] <- 0
validators$eligible_stake_mnde[validators$minimum_stake < ELIGIBILITY_MNDE_STAKE_MIN_STAKE] <- 0
validators$eligible_stake_mnde[validators$score < min_score_in_algo_set * ELIGIBILITY_MNDE_SCORE_THRESHOLD_MULTIPLIER] <- 0
validators$eligible_stake_mnde[parse_version(validators$version) < ELIGIBILITY_MIN_VERSION] <- 0 # UI hint provided earlier

for (i in 1:nrow(validators)) {
  if (validators[i, "max_commission"] > ELIGIBILITY_MNDE_STAKE_MAX_COMMISSION) {
    validators[i, "ui_hints"][[1]] <- list(c(validators[i, "ui_hints"][[1]], "NOT_ELIGIBLE_MNDE_STAKE_MAX_COMMISSION_OVER_10"))
  }
  if (validators[i, "minimum_stake"] < ELIGIBILITY_MNDE_STAKE_MIN_STAKE) {
    validators[i, "ui_hints"][[1]] <- list(c(validators[i, "ui_hints"][[1]], "NOT_ELIGIBLE_MNDE_STAKE_MIN_STAKE_BELOW_100"))
  }
  if (validators[i, "score"] < min_score_in_algo_set * ELIGIBILITY_MNDE_SCORE_THRESHOLD_MULTIPLIER) {
    validators[i, "ui_hints"][[1]] <- list(c(validators[i, "ui_hints"][[1]], "NOT_ELIGIBLE_MNDE_STAKE_SCORE_TOO_LOW"))
  }
}

# Apply eligibility on votes to get effective votes
mnde_valid_votes <- round(validators$mnde_votes * validators$eligible_stake_mnde / 1e9)

# Apply cap on the share of mnde votes
mnde_power_cap <- round(sum(mnde_valid_votes) * MNDE_VALIDATOR_CAP)
validators$mnde_power <- pmin(mnde_valid_votes, mnde_power_cap)

# Find out how much votes got truncated
mnde_overflow <- sum(mnde_valid_votes) - sum(validators$mnde_power)

# Sort validators by MNDE power
validators <- validators[order(validators$mnde_power, decreasing = T),]

# Distribute the overflow from the capping
for (v in 1:length(validators$mnde_power)) {
  validators_index <- seq(1, along.with = validators$mnde_power)
  # Ignore weights of already processed validators as they 1) already received their share from the overflow; 2) were overflowing
  moving_weights <- (validators_index > v - 1) * validators$mnde_power
  # Break the loop if no one else should receive stake from the mnde voting
  if (sum(moving_weights) == 0) {
    break
  }
  # How much should the power increase from the overflow
  mnde_power_increase <- round(mnde_overflow * moving_weights[v] / sum(moving_weights))
  # Limit the increase of mnde power if cap should be applied
  mnde_power_increase_capped <- min(mnde_power_increase, mnde_power_cap - moving_weights[v])
  # Increase mnde power for this validator
  validators$mnde_power <- validators$mnde_power + (validators_index == v) * mnde_power_increase_capped
  # Reduce the overflow by what was given to this validator
  mnde_overflow <- mnde_overflow - mnde_power_increase_capped
}

# Scale mnde power to a percentage
if (sum(validators$mnde_power) > 0) {
  total_mnde_power <- sum(validators$mnde_power, mnde_overflow)
  validators$mnde_power <- validators$mnde_power / total_mnde_power
  mnde_overflow_power <- mnde_overflow / total_mnde_power
} else {
  mnde_overflow_power <- 1
}

STAKE_CONTROL_MNDE_SOL <- TOTAL_STAKE * STAKE_CONTROL_MNDE * (1 - mnde_overflow_power)
STAKE_CONTROL_MNDE_OVERFLOW_SOL <- mnde_overflow_power * TOTAL_STAKE * STAKE_CONTROL_MNDE
STAKE_CONTROL_MSOL_SOL <- if (msol_valid_votes_total > 0) { TOTAL_STAKE * STAKE_CONTROL_MSOL } else { 0 }
STAKE_CONTROL_MSOL_UNUSED_SOL <- if (msol_valid_votes_total > 0) { 0 } else { TOTAL_STAKE * STAKE_CONTROL_MSOL }
STAKE_CONTROL_ALGO_SOL <- TOTAL_STAKE * STAKE_CONTROL_ALGO + STAKE_CONTROL_MNDE_OVERFLOW_SOL + STAKE_CONTROL_MSOL_UNUSED_SOL

validators$target_stake_mnde <- round(validators$mnde_power * STAKE_CONTROL_MNDE_SOL)
validators$target_stake_msol <- round(validators$msol_power * STAKE_CONTROL_MSOL_SOL)
validators$target_stake_algo <- round(validators$score * validators$in_algo_stake_set / sum(validators$score * validators$in_algo_stake_set) * STAKE_CONTROL_ALGO_SOL)
validators$target_stake <- validators$target_stake_mnde + validators$target_stake_algo + validators$target_stake_msol

perf_target_stake_mnde <- sum(validators$avg_adjusted_credits * validators$target_stake_mnde) / sum(validators$target_stake_mnde)
perf_target_stake_algo <- sum(validators$avg_adjusted_credits * validators$target_stake_algo) / sum(validators$target_stake_algo)
perf_target_stake_msol <- sum(validators$avg_adjusted_credits * validators$target_stake_msol) / sum(validators$target_stake_msol)

print(t(data.frame(
  TOTAL_STAKE,
  STAKE_CONTROL_MSOL_SOL,
  STAKE_CONTROL_MSOL_UNUSED_SOL,
  STAKE_CONTROL_MNDE_SOL,
  STAKE_CONTROL_MNDE_OVERFLOW_SOL,
  STAKE_CONTROL_ALGO_SOL,
  perf_target_stake_mnde,
  perf_target_stake_algo,
  perf_target_stake_msol
)))

stopifnot(TOTAL_STAKE > 3e6)
stopifnot(STAKE_CONTROL_MSOL_SOL > 900000)
stopifnot(nrow(validators) > 1000)
stopifnot(nrow(validators[validators$target_stake_algo > 0,]) == 100)

validators$ui_hints <- lapply(validators$ui_hints, paste, collapse = ',')

fwrite(validators[order(validators$rank),], file = file_out_scores, scipen = 1000, quote = T)
stakes <- validators[validators$target_stake > 0,]
fwrite(stakes[order(stakes$target_stake, decreasing = T),], file = file_out_stakes, scipen = 1000)
