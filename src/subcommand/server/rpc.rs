use super::*;
use crate::subcommand::{traits::Output as SatDetails, wallet::sats::rare_sats_from_outpoint};
use axum_jrpc::{
  error::{JsonRpcError, JsonRpcErrorReason},
  JrpcResult, JsonRpcExtractor, JsonRpcResponse,
};
use bitcoin::constants::COIN_VALUE;
use opentelemetry::trace::Tracer;
use ord_kafka_macros::trace;
use ordinals::{
  block_rarity::{
    is_palindrome, is_perfect_palindrome, is_uniform_palindrome, BLOCK286_BLOCK_HEIGHT,
    BLOCK666_BLOCK_HEIGHT, BLOCK78_BLOCK_HEIGHT, BLOCK9_450_SAT_RANGE, BLOCK9_BLOCK_HEIGHT,
    FIRST_TRANSACTION_SAT_RANGE, HITMAN_RANGE_MAP, JPEG_BLOCK_HEIGHTS, LEGACY_RANGE_MAP,
    NAKAMOTO_BLOCK_HEIGHTS, PIZZA_RANGE_MAP, TAPROOT_BLOCK_HEIGHT, VINTAGE_BLOCK_HEIGHT,
  },
  BlockRarity,
};
use serde_json::Value;
use std::cmp::{max, min};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct BlockRarityInfo {
  pub block_rarity: BlockRarity,
  pub chunks: Vec<(u64, u64)>,
}

pub(super) async fn handler(
  Extension(_page_config): Extension<Arc<ServerConfig>>,
  Extension(index): Extension<Arc<Index>>,
  value: JsonRpcExtractor,
) -> JrpcResult {
  match value.method.as_str() {
    "getHealth" => get_health(value).await,
    "getSatRanges" => get_sat_ranges(value, index).await,
    method => Ok(value.method_not_found(method)),
  }
}

fn invalid_params(answer_id: i64, message: String) -> JrpcResult {
  Err(JsonRpcResponse::error(
    answer_id,
    JsonRpcError::new(JsonRpcErrorReason::InvalidParams, message, Value::default()),
  ))
}

async fn get_health(value: JsonRpcExtractor) -> JrpcResult {
  let answer_id = value.get_answer_id();
  Ok(JsonRpcResponse::success(answer_id, "OK"))
}

#[trace]
async fn get_sat_ranges(value: JsonRpcExtractor, index: Arc<Index>) -> JrpcResult {
  #[derive(Deserialize)]
  struct Req {
    utxos: Vec<String>,
  }

  #[derive(Serialize)]
  struct SatRange {
    start: u64,
    end: u64,
    block_rarities: Vec<BlockRarityInfo>,
    block_height: Height,
    block_hash: Option<BlockHash>,
  }

  #[derive(Serialize)]
  struct RareSat {
    offset: u64,
    rarity: Rarity,
    sat: Sat,
    sat_details: SatDetails,
  }

  #[derive(Serialize)]
  struct Utxo {
    utxo: String,
    sat_ranges: Vec<SatRange>,
    rare_sats: Vec<RareSat>,
  }

  #[derive(Serialize)]
  struct Res {
    utxos: Vec<Utxo>,
  }

  let answer_id = value.get_answer_id();
  if !index.has_sat_index() {
    return invalid_params(answer_id, "Sat index is not available".to_string());
  }

  let req: Req = value.parse_params()?;
  let mut res = Res { utxos: vec![] };

  for output in req.utxos {
    let outpoint = match OutPoint::from_str(output.as_str()) {
      Ok(outpoint) => outpoint,
      Err(err) => return invalid_params(answer_id, err.to_string()),
    };
    let mut utxo = Utxo {
      utxo: output.clone(),
      sat_ranges: vec![],
      rare_sats: vec![],
    };
    let list = match index.list(outpoint) {
      Ok(list) => list,
      Err(err) => return invalid_params(answer_id, err.to_string()),
    };
    let mut sat_ranges = vec![];
    if let Some(ranges) = list {
      for range in ranges {
        let block_rarities = match get_block_rarities(range.0, range.1) {
          Ok(block_rarities) => block_rarities,
          Err(err) => return invalid_params(answer_id, err.to_string()),
        };

        let block_height = Sat(range.0).height();
        utxo.sat_ranges.push(SatRange {
          start: range.0,
          end: range.1,
          block_rarities,
          block_height,
          block_hash: index.block_hash(Some(block_height.n())).unwrap(),
        });
        sat_ranges.push(range);
      }
    }

    for (_, sat, offset, rarity) in rare_sats_from_outpoint(outpoint, sat_ranges) {
      let sat_details = SatDetails {
        number: sat.n(),
        decimal: sat.decimal().to_string(),
        degree: sat.degree().to_string(),
        name: sat.name(),
        height: sat.height().0,
        cycle: sat.cycle(),
        epoch: sat.epoch().0,
        period: sat.period(),
        offset: sat.third(),
        rarity: sat.rarity(),
      };
      utxo.rare_sats.push(RareSat {
        offset,
        rarity,
        sat,
        sat_details,
      });
    }

    res.utxos.push(utxo);
  }

  Ok(JsonRpcResponse::success(answer_id, res))
}

fn get_block_rarities(start: u64, end: u64) -> Result<Vec<BlockRarityInfo>> {
  if start >= end {
    return Err(anyhow!("invalid sat range: start {start} >= end {end}"));
  }

  if Sat(start).height().n() != Sat(end - 1).height().n() {
    return Err(anyhow!(
      "invalid sat range: start {start} and end {end} are in different blocks"
    ));
  }

  let mut block_rarities = vec![];
  for block_rarity in &[
    BlockRarity::Vintage,
    BlockRarity::Nakamoto,
    BlockRarity::Block9,
    BlockRarity::Block78,
    BlockRarity::FirstTransaction,
    BlockRarity::Pizza,
    BlockRarity::Palindrome,
    BlockRarity::Alpha,
    BlockRarity::Omega,
    BlockRarity::PerfectPalinception,
    BlockRarity::UniformPalinception,
    BlockRarity::Block286,
    BlockRarity::JPEG,
    BlockRarity::Legacy,
    BlockRarity::Hitman,
    BlockRarity::Block666,
    // BlockRarity::Taproot,
    BlockRarity::PaliblockPalindrome,
  ] {
    let rarities = get_block_rarity_chunks(block_rarity, start, end);
    for (rarity, chunks) in rarities {
      if !chunks.is_empty() {
        block_rarities.push(BlockRarityInfo {
          block_rarity: rarity,
          chunks,
        });
      }
    }
  }

  Ok(block_rarities)
}

fn get_block_rarity_chunks(
  block_rarity: &BlockRarity,
  start: u64,
  end: u64,
) -> Vec<(BlockRarity, Vec<(u64, u64)>)> {
  let mut res = vec![];
  let block_height = Sat(start).height().n();

  match block_rarity {
    BlockRarity::Vintage => {
      if block_height <= VINTAGE_BLOCK_HEIGHT {
        res.push((block_rarity.clone(), vec![(start, end)]))
      }
    }
    BlockRarity::Nakamoto => {
      if NAKAMOTO_BLOCK_HEIGHTS.contains(&block_height) {
        res.push((block_rarity.clone(), vec![(start, end)]))
      }
    }
    BlockRarity::Block9 => {
      if block_height == BLOCK9_BLOCK_HEIGHT {
        res.push((block_rarity.clone(), vec![(start, end)]));
        if start < BLOCK9_450_SAT_RANGE.1 {
          res.push((
            BlockRarity::Block9_450,
            vec![(start, min(BLOCK9_450_SAT_RANGE.1, end))],
          ))
        }
      }
    }
    BlockRarity::Block78 => {
      if block_height == BLOCK78_BLOCK_HEIGHT {
        res.push((block_rarity.clone(), vec![(start, end)]))
      }
    }
    BlockRarity::Block286 => {
      if block_height == BLOCK286_BLOCK_HEIGHT {
        res.push((block_rarity.clone(), vec![(start, end)]))
      }
    }
    BlockRarity::FirstTransaction => {
      if block_height == BLOCK9_BLOCK_HEIGHT && start < FIRST_TRANSACTION_SAT_RANGE.1 {
        res.push((
          block_rarity.clone(),
          vec![(start, min(FIRST_TRANSACTION_SAT_RANGE.1, end))],
        ));
      }
    }
    BlockRarity::Pizza => {
      let mut chunks = vec![];
      if PIZZA_RANGE_MAP.contains_key(&block_height) {
        let pizza_sat_ranges = PIZZA_RANGE_MAP.get(&block_height).unwrap();
        for range in pizza_sat_ranges {
          if (start >= range.1) || (end <= range.0) {
            continue;
          }
          chunks.push((max(range.0, start), min(range.1, end)));
        }
      }
      res.push((block_rarity.clone(), chunks));
    }
    BlockRarity::Palindrome => {
      let mut normal_chunks = vec![];
      let mut perfect_chunks = vec![];
      let mut uniform_chunks = vec![];
      let mut paliblock_chunks = vec![];
      // "end" sat is exclusive, but get_palindromes_from_sat_range wants both start and end
      // to be inclusive, a.k.a [start, end] range.
      for palindrome in get_palindromes_from_sat_range(start, end - 1) {
        normal_chunks.push((palindrome, palindrome + 1));
        if is_perfect_palindrome(&palindrome.to_string()) {
          perfect_chunks.push((palindrome, palindrome + 1));
        }
        if is_uniform_palindrome(&palindrome.to_string()) {
          uniform_chunks.push((palindrome, palindrome + 1));
        }
        if is_palindrome(&block_height.to_string()) {
          paliblock_chunks.push((palindrome, palindrome + 1));
        }
      }
      res.push((BlockRarity::Palindrome, normal_chunks));
      res.push((BlockRarity::PerfectPalinception, perfect_chunks));
      res.push((BlockRarity::UniformPalinception, uniform_chunks));
      res.push((BlockRarity::PaliblockPalindrome, paliblock_chunks));
    }
    BlockRarity::Alpha => {
      let mut chunks = vec![];
      for alpha in get_alpha_from_sat_range(start, end) {
        chunks.push((alpha, alpha + 1))
      }
      res.push((block_rarity.clone(), chunks));
    }
    BlockRarity::Omega => {
      let mut chunks = vec![];
      for omega in get_omega_from_sat_range(start, end) {
        chunks.push((omega, omega + 1))
      }
      res.push((block_rarity.clone(), chunks));
    }
    BlockRarity::JPEG => {
      if JPEG_BLOCK_HEIGHTS.contains(&block_height) {
        res.push((block_rarity.clone(), vec![(start, end)]))
      }
    }
    BlockRarity::Legacy => {
      let mut chunks = vec![];
      if LEGACY_RANGE_MAP.contains_key(&block_height) {
        let legacy_sat_ranges = LEGACY_RANGE_MAP.get(&block_height).unwrap();
        for range in legacy_sat_ranges {
          if (start >= range.1) || (end <= range.0) {
            continue;
          }
          chunks.push((max(range.0, start), min(range.1, end)));
        }
      }
      res.push((block_rarity.clone(), chunks));
    }
    BlockRarity::Hitman => {
      let mut chunks = vec![];
      if HITMAN_RANGE_MAP.contains_key(&block_height) {
        let hitman_sat_ranges = HITMAN_RANGE_MAP.get(&block_height).unwrap();
        for range in hitman_sat_ranges {
          if (start >= range.1) || (end <= range.0) {
            continue;
          }
          chunks.push((max(range.0, start), min(range.1, end)));
        }
      }
      res.push((block_rarity.clone(), chunks));
    }
    BlockRarity::Block666 => {
      if block_height == BLOCK666_BLOCK_HEIGHT {
        res.push((block_rarity.clone(), vec![(start, end)]))
      }
    }
    BlockRarity::Taproot => {
      if block_height == TAPROOT_BLOCK_HEIGHT {
        res.push((block_rarity.clone(), vec![(start, end)]))
      }
    }
    _ => { /* ignore */ }
  }
  res
}

fn get_palindromes_from_sat_range(start: u64, end: u64) -> Vec<u64> {
  let sat_range_start_string = start.to_string();
  let sat_range_end_string = end.to_string();
  let sat_range_start_length = sat_range_start_string.len();
  let sat_range_end_length = sat_range_end_string.len();

  let mut equal_length_ranges: Vec<(String, String)> = vec![];
  if sat_range_start_length == sat_range_end_length {
    equal_length_ranges.push((sat_range_start_string.clone(), sat_range_end_string.clone()));
  } else {
    equal_length_ranges.push((
      sat_range_start_string.clone(),
      "9".repeat(sat_range_start_length),
    ));

    for i in (sat_range_start_length + 1)..sat_range_end_length {
      equal_length_ranges.push(("1".to_string() + &"0".repeat(i - 1), "9".repeat(i)));
    }
    equal_length_ranges.push((
      "1".to_string() + &"0".repeat(sat_range_end_length - 1),
      sat_range_end_string.clone(),
    ));
  }

  let mut palindromes: Vec<u64> = vec![];
  for range in equal_length_ranges {
    palindromes.extend(get_palindromes_from_equal_length_range(range.0, range.1));
  }

  palindromes
}

fn get_palindromes_from_equal_length_range(start_string: String, end_string: String) -> Vec<u64> {
  let mut palindromes: Vec<u64> = vec![];

  let sat_length = start_string.len();
  let palindrome_sig_digits = (sat_length + 1) / 2;
  let middle_digit_exists = sat_length % 2 == 1;

  let start_sig_digits = &start_string[..palindrome_sig_digits];
  let start_sig_digits_number = start_sig_digits.parse::<u64>().unwrap();
  let end_sig_digits = &end_string[..palindrome_sig_digits];
  let end_sig_digits_number = end_sig_digits.parse::<u64>().unwrap();

  let start_sat = start_string.parse::<u64>().unwrap();
  let end_sat = end_string.parse::<u64>().unwrap();

  let get_palindrome = |sig_digits: &str| -> u64 {
    let palindrome_string = sig_digits.to_string()
      + &sig_digits.chars().rev().collect::<String>()[usize::from(middle_digit_exists)..];
    palindrome_string.parse::<u64>().unwrap()
  };

  let potential_first_palindrome = get_palindrome(start_sig_digits);
  if start_sat <= potential_first_palindrome && potential_first_palindrome <= end_sat {
    palindromes.push(potential_first_palindrome);
  }

  palindromes.extend(if start_sig_digits_number + 1 < end_sig_digits_number {
    (start_sig_digits_number + 1..end_sig_digits_number)
      .map(|num| get_palindrome(&num.to_string()))
      .collect::<Vec<u64>>()
  } else {
    vec![]
  });

  if start_sig_digits != end_sig_digits {
    let potential_last_palindrome = get_palindrome(end_sig_digits);
    if start_sat <= potential_last_palindrome && potential_last_palindrome <= end_sat {
      palindromes.push(potential_last_palindrome);
    }
  }
  palindromes
}

fn get_alpha_from_sat_range(start: u64, end: u64) -> Vec<u64> {
  let mut alphas: Vec<u64> = vec![];
  let mut alpha = (start - 1) / COIN_VALUE * COIN_VALUE + COIN_VALUE;
  while alpha < end {
    alphas.push(alpha);
    alpha += COIN_VALUE;
  }
  alphas
}

fn get_omega_from_sat_range(start: u64, end: u64) -> Vec<u64> {
  let mut omegas: Vec<u64> = vec![];
  let mut omega = end / COIN_VALUE * COIN_VALUE - 1;
  while omega >= start {
    omegas.push(omega);
    omega -= COIN_VALUE;
  }
  omegas
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_invalid_params() {
    let result = invalid_params(123, "Invalid input".to_string());
    assert!(result.is_err());
    let error = result.err().unwrap();
    assert_eq!(error.id, 123);
  }

  #[tokio::test]
  async fn test_get_health() {
    let value = JsonRpcExtractor {
      method: "getHealth".to_string(),
      parsed: Value::default(),
      id: 0,
    };
    let result = get_health(value).await;
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response.id, 0);
  }

  #[test]
  fn test_get_block_rarities() {
    let mut block_rarities =
      get_block_rarities(460 * COIN_VALUE - 10_000, 460 * COIN_VALUE + 10_000).unwrap();
    assert_eq!(
      block_rarities,
      vec![
        BlockRarityInfo {
          block_rarity: BlockRarity::Vintage,
          chunks: vec![(460 * COIN_VALUE - 10_000, 460 * COIN_VALUE + 10_000)]
        },
        BlockRarityInfo {
          block_rarity: BlockRarity::Nakamoto,
          chunks: vec![(460 * COIN_VALUE - 10_000, 460 * COIN_VALUE + 10_000)]
        },
        BlockRarityInfo {
          block_rarity: BlockRarity::Block9,
          chunks: vec![(460 * COIN_VALUE - 10_000, 460 * COIN_VALUE + 10_000)]
        },
        BlockRarityInfo {
          block_rarity: BlockRarity::FirstTransaction,
          chunks: vec![(460 * COIN_VALUE - 10_000, 460 * COIN_VALUE)]
        },
        BlockRarityInfo {
          block_rarity: BlockRarity::Palindrome,
          chunks: vec![
            (45_999_999_954, 45_999_999_955),
            (46_000_000_064, 46_000_000_065)
          ]
        },
        BlockRarityInfo {
          block_rarity: BlockRarity::PaliblockPalindrome,
          chunks: vec![(45999999954, 45999999955), (46000000064, 46000000065)]
        },
        BlockRarityInfo {
          block_rarity: BlockRarity::Alpha,
          chunks: vec![(46_000_000_000, 46_000_000_001)]
        },
        BlockRarityInfo {
          block_rarity: BlockRarity::Omega,
          chunks: vec![(45_999_999_999, 46_000_000_000),]
        },
      ]
    );

    block_rarities =
      get_block_rarities(451 * COIN_VALUE - 10_000, 451 * COIN_VALUE + 10_000).unwrap();
    assert_eq!(
      block_rarities,
      vec![
        BlockRarityInfo {
          block_rarity: BlockRarity::Vintage,
          chunks: vec![(451 * COIN_VALUE - 10_000, 451 * COIN_VALUE + 10_000)]
        },
        BlockRarityInfo {
          block_rarity: BlockRarity::Nakamoto,
          chunks: vec![(451 * COIN_VALUE - 10_000, 451 * COIN_VALUE + 10_000)]
        },
        BlockRarityInfo {
          block_rarity: BlockRarity::Block9,
          chunks: vec![(451 * COIN_VALUE - 10_000, 451 * COIN_VALUE + 10_000)]
        },
        BlockRarityInfo {
          block_rarity: BlockRarity::Block9_450,
          chunks: vec![(451 * COIN_VALUE - 10_000, 451 * COIN_VALUE)]
        },
        BlockRarityInfo {
          block_rarity: BlockRarity::FirstTransaction,
          chunks: vec![(451 * COIN_VALUE - 10_000, 451 * COIN_VALUE + 10_000)]
        },
        BlockRarityInfo {
          block_rarity: BlockRarity::Palindrome,
          chunks: vec![(45099999054, 45099999055), (45100000154, 45100000155)]
        },
        BlockRarityInfo {
          block_rarity: BlockRarity::PaliblockPalindrome,
          chunks: vec![(45099999054, 45099999055), (45100000154, 45100000155)]
        },
        BlockRarityInfo {
          block_rarity: BlockRarity::Alpha,
          chunks: vec![(45_100_000_000, 45_100_000_001)]
        },
        BlockRarityInfo {
          block_rarity: BlockRarity::Omega,
          chunks: vec![(45_099_999_999, 45_100_000_000),]
        },
      ]
    );

    block_rarities =
      get_block_rarities(78 * 50 * COIN_VALUE + 10_000, 78 * 50 * COIN_VALUE + 20_000).unwrap();
    assert_eq!(
      block_rarities,
      vec![
        BlockRarityInfo {
          block_rarity: BlockRarity::Vintage,
          chunks: vec![(78 * 50 * COIN_VALUE + 10_000, 78 * 50 * COIN_VALUE + 20_000)]
        },
        BlockRarityInfo {
          block_rarity: BlockRarity::Block78,
          chunks: vec![(78 * 50 * COIN_VALUE + 10_000, 78 * 50 * COIN_VALUE + 20_000)]
        },
      ]
    );

    block_rarities = get_block_rarities(
      286 * 50 * COIN_VALUE + 10_000,
      286 * 50 * COIN_VALUE + 20_000,
    )
    .unwrap();
    assert_eq!(
      block_rarities,
      vec![
        BlockRarityInfo {
          block_rarity: BlockRarity::Vintage,
          chunks: vec![(
            286 * 50 * COIN_VALUE + 10_000,
            286 * 50 * COIN_VALUE + 20_000
          )]
        },
        BlockRarityInfo {
          block_rarity: BlockRarity::Nakamoto,
          chunks: vec![(
            286 * 50 * COIN_VALUE + 10_000,
            286 * 50 * COIN_VALUE + 20_000
          )]
        },
        BlockRarityInfo {
          block_rarity: BlockRarity::Block286,
          chunks: vec![(
            286 * 50 * COIN_VALUE + 10_000,
            286 * 50 * COIN_VALUE + 20_000
          )]
        },
      ]
    );

    block_rarities = get_block_rarities(3153515_5000000, 3153515_6000000).unwrap();
    assert_eq!(
      block_rarities,
      vec![BlockRarityInfo {
        block_rarity: BlockRarity::Palindrome,
        chunks: vec![(31535155153513, 31535155153514)],
      }]
    );

    block_rarities = get_block_rarities(40004_11111_00000, 40004_11112_00000).unwrap();
    assert_eq!(
      block_rarities,
      vec![
        BlockRarityInfo {
          block_rarity: BlockRarity::Palindrome,
          chunks: vec![(400041111140004, 400041111140005)],
        },
        BlockRarityInfo {
          block_rarity: BlockRarity::UniformPalinception,
          chunks: vec![(400041111140004, 400041111140005)],
        },
        BlockRarityInfo {
          block_rarity: BlockRarity::PaliblockPalindrome,
          chunks: vec![(400041111140004, 400041111140005)],
        },
      ]
    );

    block_rarities = get_block_rarities(7685867_0000000, 7685868_0000000).unwrap();
    assert_eq!(
      block_rarities,
      vec![
        BlockRarityInfo {
          block_rarity: BlockRarity::Palindrome,
          chunks: vec![(76858677685867, 76858677685868)],
        },
        BlockRarityInfo {
          block_rarity: BlockRarity::PerfectPalinception,
          chunks: vec![(76858677685867, 76858677685868)],
        },
        BlockRarityInfo {
          block_rarity: BlockRarity::UniformPalinception,
          chunks: vec![(76858677685867, 76858677685868)],
        },
      ]
    );

    block_rarities = get_block_rarities(3153515_5000000, 3153515_6000000).unwrap();
    assert_eq!(
      block_rarities,
      vec![BlockRarityInfo {
        block_rarity: BlockRarity::Palindrome,
        chunks: vec![(31535155153513, 31535155153514)],
      }]
    );

    block_rarities = get_block_rarities(204589006000000, 204589046000000).unwrap();
    assert_eq!(
      block_rarities,
      vec![
        BlockRarityInfo {
          block_rarity: BlockRarity::Pizza,
          chunks: vec![
            (204589006000000, 204589008000000),
            (204589017000000, 204589019000000),
            (204589026000000, 204589028000000),
            (204589029000000, 204589030000000),
            (204589032000000, 204589033000000),
            (204589034000000, 204589035000000),
            (204589037000000, 204589038000000),
            (204589041000000, 204589043000000),
            (204589045000000, 204589046000000)
          ]
        },
        BlockRarityInfo {
          block_rarity: BlockRarity::Palindrome,
          chunks: vec![
            (204589010985402, 204589010985403),
            (204589020985402, 204589020985403),
            (204589030985402, 204589030985403),
            (204589040985402, 204589040985403)
          ]
        }
      ]
    );

    block_rarities = get_block_rarities(162064056592929, 162064057592929).unwrap();
    assert_eq!(
      block_rarities,
      vec![BlockRarityInfo {
        block_rarity: BlockRarity::JPEG,
        chunks: vec![(162064056592929, 162064057592929),]
      },]
    );
  }

  #[test]
  fn test_legacy_range() {
    let block_rarities = get_block_rarities(1425808476860827, 1425808478101389).unwrap();
    assert_eq!(
      block_rarities,
      vec![BlockRarityInfo {
        block_rarity: BlockRarity::Legacy,
        chunks: vec![
          (1425808476860827, 1425808476946804),
          (1425808477861389, 1425808478101389),
        ]
      },]
    );
  }

  #[test]
  fn test_hitman_range() {
    let block_rarities = get_block_rarities(500438693191798, 500438695191798).unwrap();
    assert_eq!(
      block_rarities,
      vec![BlockRarityInfo {
        block_rarity: BlockRarity::Hitman,
        chunks: vec![(500438693191798, 500438695191798),]
      },]
    );
  }

  // #[test]
  // fn test_taproot_range() {
  //   let block_rarities = get_block_rarities(1887270184000002, 1887270184000602).unwrap();
  //   assert_eq!(
  //     block_rarities,
  //     vec![BlockRarityInfo {
  //       block_rarity: BlockRarity::Taproot,
  //       chunks: vec![(1887270184000002, 1887270184000602),]
  //     },]
  //   );
  // }

  #[test]
  fn test_block666_range() {
    let block_rarities =
      get_block_rarities(666 * 50 * COIN_VALUE + 1000, 666 * 50 * COIN_VALUE + 2000).unwrap();
    assert_eq!(
      block_rarities,
      vec![
        BlockRarityInfo {
          block_rarity: BlockRarity::Vintage,
          chunks: vec![(666 * 50 * COIN_VALUE + 1000, 666 * 50 * COIN_VALUE + 2000),]
        },
        BlockRarityInfo {
          block_rarity: BlockRarity::Block666,
          chunks: vec![(666 * 50 * COIN_VALUE + 1000, 666 * 50 * COIN_VALUE + 2000),]
        },
      ]
    );
  }

  #[test]
  fn test_get_palindromes_from_sat_range() {
    env_logger::init();
    let mut palindromes = get_palindromes_from_sat_range(1, 999);
    assert_eq!(
      palindromes,
      vec![
        1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 22, 33, 44, 55, 66, 77, 88, 99, 101, 111, 121, 131, 141,
        151, 161, 171, 181, 191, 202, 212, 222, 232, 242, 252, 262, 272, 282, 292, 303, 313, 323,
        333, 343, 353, 363, 373, 383, 393, 404, 414, 424, 434, 444, 454, 464, 474, 484, 494, 505,
        515, 525, 535, 545, 555, 565, 575, 585, 595, 606, 616, 626, 636, 646, 656, 666, 676, 686,
        696, 707, 717, 727, 737, 747, 757, 767, 777, 787, 797, 808, 818, 828, 838, 848, 858, 868,
        878, 888, 898, 909, 919, 929, 939, 949, 959, 969, 979, 989, 999
      ]
    );
    palindromes = get_palindromes_from_sat_range(3153515_5000000, 3153515_6000000);
    assert_eq!(palindromes, vec![31535155153513]);
    palindromes = get_palindromes_from_sat_range(1999999_9999999, 2000000_0999999);
    assert_eq!(palindromes, vec![20000000000002]);
    palindromes = get_palindromes_from_sat_range(3153515_6000000, 3153515_7000000);
    assert_eq!(palindromes.len(), 0);
  }
}
