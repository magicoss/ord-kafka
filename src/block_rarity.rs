use super::*;
use lazy_static::lazy_static;
use std::collections::HashMap;

#[derive(Debug, PartialEq, PartialOrd, Clone)]
pub enum BlockRarity {
  Vintage,
  Nakamoto,
  FirstTransaction,
  Pizza,
  Block9,
  Block78,
  Palindrome,
}

impl Display for BlockRarity {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    write!(
      f,
      "{}",
      match self {
        Self::Vintage => "vintage",
        Self::Nakamoto => "nakamoto",
        Self::FirstTransaction => "firsttransaction",
        Self::Palindrome => "palindrome",
        Self::Pizza => "pizza",
        Self::Block9 => "block9",
        Self::Block78 => "block78",
      }
    )
  }
}

impl From<Sat> for Vec<BlockRarity> {
  fn from(sat: Sat) -> Self {
    let mut res = Vec::<BlockRarity>::new();
    let block_height = sat.height().n();

    if block_height <= MAX_PIZZA_BLOCK_HEIGHT {
      if block_height <= VINTAGE_BLOCK_HEIGHT {
        res.push(BlockRarity::Vintage);
      }
      if NAKAMOTO_BLOCK_HEIGHTS.contains(&block_height) {
        res.push(BlockRarity::Nakamoto);
      }
      if is_pizza_sat(&sat) {
        res.push(BlockRarity::Pizza);
      }
      if block_height == BLOCK9_BLOCK_HEIGHT {
        if sat.n() >= FIRST_TRANSACTION_SAT_RANGE.0 && sat.n() < FIRST_TRANSACTION_SAT_RANGE.1 {
          res.push(BlockRarity::FirstTransaction);
        }
        res.push(BlockRarity::Block9);
      } else if block_height == BLOCK78_BLOCK_HEIGHT {
        res.push(BlockRarity::Block78);
      }
    }

    if is_palindrome(&sat.n()) {
      res.push(BlockRarity::Palindrome);
    }
    res
  }
}

impl FromStr for BlockRarity {
  type Err = Error;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "vintage" => Ok(Self::Vintage),
      "nakamoto" => Ok(Self::Nakamoto),
      "firsttransaction" => Ok(Self::FirstTransaction),
      "palindrome" => Ok(Self::Palindrome),
      "pizza" => Ok(Self::Pizza),
      "block9" => Ok(Self::Block9),
      "block78" => Ok(Self::Block78),
      _ => Err(anyhow!("invalid rarity: {s}")),
    }
  }
}

impl Serialize for BlockRarity {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    serializer.collect_str(self)
  }
}

impl<'de> Deserialize<'de> for BlockRarity {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    Ok(DeserializeFromStr::deserialize(deserializer)?.0)
  }
}

pub(crate) fn is_palindrome(n: &u64) -> bool {
  let s = n.to_string();
  if s.chars().next() != s.chars().last() {
    return false;
  }
  let reversed = s.chars().rev().collect::<String>();
  s == reversed
}

fn in_range(n: &u64, ranges: &Vec<(u64, u64)>) -> bool {
  for range in ranges {
    if n >= &range.0 && n < &range.1 {
      return true;
    }
  }
  false
}

fn is_pizza_sat(sat: &Sat) -> bool {
  let block_height = sat.height().n();

  if PIZZA_RANGE_MAP.contains_key(&block_height) {
    let pizza_sat_range = PIZZA_RANGE_MAP.get(&block_height).unwrap();
    return in_range(&sat.n(), pizza_sat_range);
  }
  false
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_is_palindrome() {
    assert!(is_palindrome(&164114646411461u64));
    assert!(!is_palindrome(&164114646411462u64));
  }

  #[test]
  fn block_rarities() {
    assert_eq!(
      Sat(1).block_rarities(),
      [BlockRarity::Vintage, BlockRarity::Palindrome]
    );
    assert_eq!(
      Sat(1).block_rarities(),
      [BlockRarity::Vintage, BlockRarity::Palindrome]
    );
    assert_eq!(Sat(1000).block_rarities(), [BlockRarity::Vintage]);
    assert_eq!(
      Sat(1430418430854).block_rarities(),
      [BlockRarity::Vintage, BlockRarity::Nakamoto]
    );
    assert_eq!(
      Sat(45017789073).block_rarities(),
      [
        BlockRarity::Vintage,
        BlockRarity::Nakamoto,
        BlockRarity::FirstTransaction,
        BlockRarity::Block9
      ]
    );
    assert_eq!(
      Sat(392664457568).block_rarities(),
      [BlockRarity::Vintage, BlockRarity::Block78]
    );
    assert_eq!(Sat(263080763952535).block_rarities(), [BlockRarity::Pizza]);
    assert_eq!(
      Sat(874357828753478).block_rarities(),
      [BlockRarity::Palindrome]
    );
    assert_eq!(
      Sat(120488565884021).block_rarities(),
      [BlockRarity::Pizza, BlockRarity::Palindrome]
    );
    assert_eq!(Sat(463080763952535).block_rarities(), []);
  }

  #[test]
  fn from_str_and_deserialize_ok() {
    #[track_caller]
    fn case(s: &str, expected: BlockRarity) {
      let actual = s.parse::<BlockRarity>().unwrap();
      assert_eq!(actual, expected);
      let round_trip = actual.to_string().parse::<BlockRarity>().unwrap();
      assert_eq!(round_trip, expected);
      let serialized = serde_json::to_string(&expected).unwrap();
      assert!(serde_json::from_str::<BlockRarity>(&serialized).is_ok());
    }

    case("vintage", BlockRarity::Vintage);
    case("nakamoto", BlockRarity::Nakamoto);
    case("firsttransaction", BlockRarity::FirstTransaction);
    case("pizza", BlockRarity::Pizza);
    case("block9", BlockRarity::Block9);
    case("block78", BlockRarity::Block78);
    case("palindrome", BlockRarity::Palindrome);
  }

  #[test]
  fn from_str_err() {
    "abc".parse::<BlockRarity>().unwrap_err();

    "".parse::<BlockRarity>().unwrap_err();
  }

  #[test]
  fn test_is_pizza_sat() {
    assert!(is_pizza_sat(&Sat(120485000000000)));
    assert!(!is_pizza_sat(&Sat(120475000000000)));
    assert!(!is_pizza_sat(&Sat(120495000000000)));
    // ranges in block 40918
    assert!(is_pizza_sat(&Sat(204589179000000)));
    assert!(is_pizza_sat(&Sat(204589184000001)));
    assert!(is_pizza_sat(&Sat(204589186000002)));
    assert!(is_pizza_sat(&Sat(204589199000003)));
    assert!(!is_pizza_sat(&Sat(204589182000003)));
    assert!(!is_pizza_sat(&Sat(204589185000002)));
    assert!(!is_pizza_sat(&Sat(204589189000001)));
    assert!(!is_pizza_sat(&Sat(204589200000000)));
  }
}

pub const MAX_PIZZA_BLOCK_HEIGHT: u32 = 56788;
pub const VINTAGE_BLOCK_HEIGHT: u32 = 1000;
pub const BLOCK9_BLOCK_HEIGHT: u32 = 9;
pub const BLOCK78_BLOCK_HEIGHT: u32 = 78;
pub const NAKAMOTO_BLOCK_HEIGHTS: [u32; 19] = [
  9, 286, 688, 877, 1760, 2459, 2485, 3479, 5326, 9443, 9925, 10645, 14450, 15625, 15817, 19093,
  23014, 28593, 29097,
];
pub const FIRST_TRANSACTION_SAT_RANGE: (u64, u64) = (45000000000, 46000000000);

lazy_static! {
  pub static ref PIZZA_RANGE_MAP: HashMap<u32, Vec<(u64, u64)>> = {
    let mut map = HashMap::new();
    for (start, end) in PIZZA_RANGES {
      let block_height = u32::try_from(start / (50 * COIN_VALUE)).unwrap();
      let ranges = map.entry(block_height).or_insert(vec![]);
      ranges.push((start, end));
    }
    map
  };
}

const PIZZA_RANGES: [(u64, u64); 847] = [
  (120485000000000, 120490000000000),
  (155900000000000, 155905000000000),
  (156145000000000, 156150000000000),
  (156235000000000, 156240000000000),
  (156375000000000, 156380000000000),
  (157020000000000, 157025000000000),
  (181804571000000, 181804572000000),
  (181804573000000, 181804575000000),
  (181804576000000, 181804577000000),
  (181804582000000, 181804583000000),
  (181804584000000, 181804586000000),
  (181804587000000, 181804589000000),
  (181804592000000, 181804593000000),
  (181804594000000, 181804595000000),
  (184110000000000, 184115000000000),
  (186165000000000, 186170000000000),
  (187189895000000, 187190000000000),
  (187755000000000, 187757495000000),
  (192020000000000, 192025000000000),
  (193765000000000, 193765595000000),
  (198831795000000, 198835000000000),
  (198955000000000, 198960000000000),
  (200396095000000, 200396295000000),
  (202325000000000, 202330000000000),
  (202475000000000, 202480000000000),
  (202705000000000, 202710000000000),
  (203485000000000, 203490000000000),
  (204585000000000, 204589000000000),
  (204589002000000, 204589003000000),
  (204589004000000, 204589005000000),
  (204589006000000, 204589008000000),
  (204589017000000, 204589019000000),
  (204589026000000, 204589028000000),
  (204589029000000, 204589030000000),
  (204589032000000, 204589033000000),
  (204589034000000, 204589035000000),
  (204589037000000, 204589038000000),
  (204589041000000, 204589043000000),
  (204589045000000, 204589046000000),
  (204589061000000, 204589062000000),
  (204589064000000, 204589065000000),
  (204589066000000, 204589068000000),
  (204589075000000, 204589077000000),
  (204589080000000, 204589081000000),
  (204589089000000, 204589090000000),
  (204589098000000, 204589100000000),
  (204589102000000, 204589103000000),
  (204589106000000, 204589107000000),
  (204589108000000, 204589109000000),
  (204589112000000, 204589113000000),
  (204589116000000, 204589117000000),
  (204589119000000, 204589120000000),
  (204589121000000, 204589122000000),
  (204589125000000, 204589126000000),
  (204589131000000, 204589132000000),
  (204589136000000, 204589137000000),
  (204589139000000, 204589140000000),
  (204589147000000, 204589148000000),
  (204589151000000, 204589152000000),
  (204589157000000, 204589158000000),
  (204589161000000, 204589165000000),
  (204589166000000, 204589167000000),
  (204589170000000, 204589172000000),
  (204589174000000, 204589176000000),
  (204589179000000, 204589182000000),
  (204589184000000, 204589185000000),
  (204589186000000, 204589189000000),
  (204589199000000, 204589200000000),
  (204589201000000, 204589203000000),
  (204589206000000, 204589208000000),
  (204589214000000, 204589216000000),
  (204589223000000, 204589226000000),
  (204589234000000, 204589236000000),
  (204589241000000, 204589243000000),
  (204589244000000, 204589245000000),
  (204589252000000, 204589253000000),
  (204589260000000, 204589261000000),
  (204589262000000, 204589264000000),
  (204589266000000, 204589268000000),
  (204589269000000, 204589270000000),
  (204589271000000, 204589272000000),
  (204589275000000, 204589276000000),
  (204589277000000, 204589278000000),
  (204589281000000, 204589283000000),
  (204589284000000, 204589285000000),
  (204589288000000, 204589289000000),
  (204589290000000, 204589292000000),
  (204589294000000, 204589296000000),
  (204589297000000, 204589299000000),
  (204589301000000, 204589302000000),
  (204589303000000, 204589304000000),
  (204589305000000, 204589306000000),
  (204589308000000, 204589310000000),
  (204589312000000, 204589313000000),
  (204589314000000, 204589316000000),
  (204589317000000, 204589319000000),
  (204589320000000, 204589321000000),
  (204589326000000, 204589327000000),
  (204589333000000, 204589334000000),
  (204589335000000, 204589336000000),
  (204589337000000, 204589338000000),
  (204589347000000, 204589348000000),
  (204589354000000, 204589355000000),
  (204589362000000, 204589364000000),
  (204589382000000, 204589383000000),
  (204589388000000, 204589389000000),
  (204589391000000, 204589392000000),
  (204589397000000, 204589398000000),
  (204589402000000, 204589403000000),
  (204589410000000, 204589411000000),
  (204589412000000, 204589419000000),
  (204589424000000, 204589425000000),
  (204589439000000, 204589440000000),
  (204589441000000, 204589442000000),
  (204589443000000, 204589444000000),
  (204589445000000, 204589446000000),
  (204589447000000, 204589448000000),
  (204589450000000, 204589453000000),
  (204589454000000, 204589456000000),
  (204589457000000, 204589460000000),
  (204589463000000, 204589464000000),
  (204589465000000, 204589470000000),
  (204589474000000, 204589475000000),
  (204589484000000, 204589485000000),
  (204589487000000, 204589489000000),
  (204589492000000, 204589495000000),
  (204589497000000, 204589498000000),
  (204589503000000, 204589504000000),
  (204589507000000, 204589509000000),
  (204589511000000, 204589515000000),
  (204589517000000, 204589518000000),
  (204589523000000, 204589524000000),
  (204589526000000, 204589527000000),
  (204589531000000, 204589532000000),
  (204589540000000, 204589541000000),
  (204589543000000, 204589545000000),
  (204589546000000, 204589548000000),
  (204589549000000, 204589550000000),
  (204589551000000, 204589552000000),
  (204589556000000, 204589559000000),
  (204589562000000, 204589563000000),
  (204589564000000, 204589565000000),
  (204589570000000, 204589571000000),
  (204589572000000, 204589573000000),
  (204589575000000, 204589577000000),
  (204589579000000, 204589580000000),
  (204589584000000, 204589585000000),
  (204589592000000, 204589595000000),
  (204589596000000, 204589597000000),
  (204589606000000, 204589607000000),
  (204589608000000, 204589609000000),
  (204589611000000, 204589612000000),
  (204589615000000, 204589616000000),
  (204589617000000, 204589619000000),
  (204589624000000, 204589625000000),
  (204589626000000, 204589627000000),
  (204589632000000, 204589634000000),
  (204589636000000, 204589638000000),
  (204589642000000, 204589643000000),
  (204589645000000, 204589647000000),
  (204589650000000, 204589651000000),
  (204589657000000, 204589658000000),
  (204589676000000, 204589677000000),
  (204589679000000, 204589680000000),
  (204589688000000, 204589689000000),
  (204589691000000, 204589694000000),
  (204589708000000, 204589709000000),
  (204589717000000, 204589718000000),
  (204589719000000, 204589721000000),
  (204589724000000, 204589725000000),
  (204589727000000, 204589729000000),
  (204589731000000, 204589732000000),
  (204589734000000, 204589735000000),
  (204589742000000, 204589743000000),
  (204589747000000, 204589748000000),
  (204589758000000, 204589760000000),
  (204589764000000, 204589765000000),
  (204589766000000, 204589768000000),
  (204589769000000, 204589770000000),
  (204589771000000, 204589774000000),
  (204589780000000, 204589781000000),
  (204589782000000, 204589786000000),
  (204589787000000, 204589788000000),
  (204589792000000, 204589793000000),
  (204589797000000, 204589798000000),
  (204589799000000, 204589801000000),
  (204589805000000, 204589806000000),
  (204589807000000, 204589808000000),
  (204589810000000, 204589811000000),
  (204589812000000, 204589813000000),
  (204589817000000, 204589818000000),
  (204589821000000, 204589824000000),
  (204589832000000, 204589833000000),
  (204589834000000, 204589835000000),
  (204589839000000, 204589840000000),
  (204589846000000, 204589848000000),
  (204589856000000, 204589857000000),
  (204589858000000, 204589859000000),
  (204589863000000, 204589864000000),
  (204589865000000, 204589866000000),
  (204589869000000, 204589870000000),
  (204589873000000, 204589874000000),
  (204589875000000, 204589876000000),
  (204589883000000, 204589884000000),
  (204589886000000, 204589888000000),
  (204589889000000, 204589890000000),
  (204589891000000, 204589892000000),
  (204589893000000, 204589894000000),
  (204589898000000, 204589899000000),
  (204589900000000, 204589901000000),
  (204589902000000, 204589905000000),
  (204589906000000, 204589912000000),
  (204589915000000, 204589916000000),
  (204589917000000, 204589918000000),
  (204589919000000, 204589921000000),
  (204589924000000, 204589925000000),
  (204589927000000, 204589928000000),
  (204589930000000, 204589931000000),
  (204589933000000, 204589934000000),
  (204589937000000, 204589939000000),
  (204589943000000, 204589945000000),
  (204589949000000, 204589950000000),
  (204589952000000, 204589953000000),
  (204589955000000, 204589956000000),
  (204589959000000, 204589961000000),
  (204589963000000, 204589964000000),
  (204589968000000, 204589969000000),
  (204589971000000, 204589972000000),
  (204589975000000, 204589976000000),
  (204589984000000, 204589987000000),
  (204589988000000, 204589989000000),
  (204589990000000, 204589992000000),
  (204589998000000, 204589999000000),
  (204595000000000, 204600000000000),
  (205665000000000, 205670000000000),
  (206160000000000, 206165000000000),
  (233324700000000, 233325000000000),
  (233491541000000, 233491542000000),
  (233491544000000, 233491545000000),
  (233491551000000, 233491552000000),
  (233491554000000, 233491555000000),
  (233491556000000, 233491558000000),
  (233491562000000, 233491563000000),
  (233491565000000, 233491566000000),
  (233491567000000, 233491568000000),
  (233491572000000, 233491574000000),
  (233491577000000, 233491578000000),
  (233491582000000, 233491583000000),
  (233491592000000, 233491593000000),
  (233491595000000, 233491596000000),
  (233491597000000, 233491598000000),
  (233491601000000, 233491602000000),
  (233491608000000, 233491609000000),
  (233491612000000, 233491613000000),
  (233491619000000, 233491620000000),
  (233491622000000, 233491625000000),
  (233491626000000, 233491627000000),
  (233491629000000, 233491630000000),
  (233491631000000, 233491632000000),
  (233491642000000, 233491643000000),
  (233491646000000, 233491647000000),
  (233491649000000, 233491650000000),
  (233491653000000, 233491654000000),
  (233491661000000, 233491662000000),
  (233491663000000, 233491665000000),
  (233491666000000, 233491668000000),
  (233491669000000, 233491670000000),
  (233491671000000, 233491673000000),
  (233491674000000, 233491676000000),
  (233491679000000, 233491680000000),
  (233491682000000, 233491683000000),
  (233491688000000, 233491689000000),
  (233491693000000, 233491694000000),
  (233491695000000, 233491696000000),
  (233491700000000, 233491704000000),
  (233491707000000, 233491709000000),
  (233491715000000, 233491716000000),
  (233491724000000, 233491726000000),
  (233491727000000, 233491729000000),
  (233491736000000, 233491737000000),
  (233491739000000, 233492240000000),
  (233492246000000, 233492248000000),
  (233492249000000, 233492250000000),
  (233492252000000, 233492253000000),
  (233492260000000, 233492261000000),
  (233492263000000, 233492266000000),
  (233492274000000, 233492275000000),
  (233492276000000, 233492277000000),
  (233492278000000, 233492280000000),
  (233492281000000, 233492282000000),
  (233492285000000, 233492286000000),
  (233492287000000, 233492289000000),
  (233492290000000, 233492291000000),
  (233492292000000, 233492293000000),
  (233492299000000, 233492300000000),
  (233492304000000, 233492305000000),
  (233492312000000, 233492313000000),
  (233492317000000, 233492319000000),
  (233492320000000, 233492322000000),
  (233492324000000, 233492326000000),
  (233492333000000, 233492334000000),
  (233492338000000, 233492339000000),
  (233492342000000, 233492343000000),
  (233492350000000, 233492351000000),
  (233492352000000, 233492353000000),
  (233492359000000, 233492361000000),
  (233492364000000, 233492366000000),
  (233492368000000, 233492369000000),
  (233492370000000, 233492371000000),
  (233492373000000, 233492374000000),
  (233492376000000, 233492377000000),
  (233492383000000, 233492384000000),
  (233492393000000, 233492394000000),
  (233492402000000, 233492403000000),
  (233492408000000, 233492409000000),
  (233492410000000, 233492411000000),
  (233492416000000, 233492417000000),
  (233492418000000, 233492419000000),
  (233492423000000, 233492426000000),
  (233492429000000, 233492430000000),
  (233492434000000, 233492437000000),
  (233492446000000, 233492447000000),
  (233492448000000, 233492450000000),
  (233492452000000, 233492453000000),
  (233492455000000, 233492457000000),
  (233492466000000, 233492467000000),
  (233492468000000, 233492469000000),
  (233492472000000, 233492473000000),
  (233492475000000, 233492476000000),
  (233492478000000, 233492479000000),
  (233492481000000, 233492482000000),
  (233492483000000, 233492485000000),
  (233492489000000, 233492490000000),
  (233492494000000, 233492495000000),
  (233492497000000, 233492499000000),
  (233492501000000, 233492503000000),
  (233492514000000, 233492515000000),
  (233492522000000, 233492523000000),
  (233492527000000, 233492530000000),
  (233492531000000, 233492532000000),
  (233492535000000, 233492536000000),
  (233492540000000, 233495000000000),
  (235110000000000, 235115000000000),
  (235675000000000, 235676240000000),
  (235676281000000, 235676541000000),
  (235676543000000, 235676545000000),
  (235676552000000, 235676553000000),
  (235676554000000, 235676555000000),
  (235676559000000, 235676560000000),
  (235676565000000, 235676566000000),
  (235676567000000, 235676569000000),
  (235676570000000, 235676571000000),
  (235676573000000, 235676574000000),
  (235676575000000, 235676577000000),
  (235676578000000, 235676579000000),
  (235676580000000, 235676581000000),
  (235676582000000, 235676583000000),
  (235676585000000, 235676586000000),
  (235676587000000, 235676588000000),
  (235676590000000, 235676591000000),
  (235676593000000, 235676595000000),
  (235676599000000, 235676600000000),
  (235676601000000, 235676602000000),
  (235676613000000, 235676614000000),
  (235676623000000, 235676624000000),
  (235676625000000, 235676626000000),
  (235676635000000, 235677240000000),
  (237545000000000, 237550000000000),
  (237775000000000, 237780000000000),
  (237795000000000, 237800000000000),
  (239530000000000, 239535000000000),
  (239590000000000, 239595000000000),
  (239655000000000, 239660000000000),
  (239765000000000, 239770000000000),
  (240205000000000, 240210000000000),
  (241070000000000, 241075000000000),
  (243435000000000, 243440000000000),
  (243930000000000, 243935000000000),
  (244310000000000, 244315000000000),
  (246210000000000, 246210536000000),
  (246210541000000, 246211424000000),
  (246211426000000, 246211427000000),
  (246211436000000, 246211440000000),
  (246211441000000, 246211442000000),
  (246211444000000, 246211445000000),
  (246211460000000, 246211462000000),
  (246211465000000, 246211466000000),
  (246211473000000, 246211474000000),
  (246211477000000, 246211478000000),
  (246211479000000, 246211480000000),
  (246211484000000, 246211485000000),
  (246211487000000, 246211490000000),
  (246211495000000, 246211497000000),
  (246211500000000, 246211502000000),
  (246211507000000, 246211508000000),
  (246211512000000, 246211513000000),
  (246211514000000, 246211515000000),
  (246211518000000, 246211520000000),
  (246211525000000, 246211526000000),
  (246211532000000, 246211533000000),
  (246211535000000, 246211536000000),
  (246211539000000, 246211541000000),
  (248800000000000, 248805000000000),
  (248855000000000, 248860000000000),
  (249050000000000, 249055000000000),
  (249175000000000, 249176301000000),
  (249176303000000, 249176304000000),
  (249176306000000, 249176307000000),
  (249176313000000, 249176314000000),
  (249176319000000, 249176321000000),
  (249176323000000, 249176324000000),
  (249176325000000, 249176326000000),
  (249176329000000, 249176330000000),
  (249176348000000, 249176349000000),
  (249176352000000, 249176353000000),
  (249176354000000, 249176356000000),
  (249176358000000, 249176359000000),
  (249176362000000, 249176363000000),
  (249176366000000, 249176367000000),
  (249176371000000, 249176372000000),
  (249176374000000, 249176375000000),
  (249176378000000, 249176379000000),
  (249176386000000, 249176387000000),
  (249176389000000, 249176390000000),
  (249176394000000, 249176395000000),
  (249176396000000, 249176397000000),
  (249176399000000, 249176400000000),
  (249176402000000, 249176403000000),
  (249176404000000, 249176406000000),
  (249176407000000, 249176408000000),
  (249176412000000, 249176413000000),
  (249176415000000, 249176418000000),
  (249176420000000, 249176421000000),
  (249176435000000, 249176436000000),
  (249176438000000, 249176440000000),
  (249176448000000, 249176449000000),
  (249176450000000, 249176451000000),
  (249176457000000, 249176461000000),
  (249176465000000, 249176466000000),
  (249176467000000, 249176469000000),
  (249176476000000, 249176477000000),
  (249176480000000, 249176481000000),
  (249176484000000, 249176485000000),
  (249176486000000, 249176488000000),
  (249176497000000, 249176499000000),
  (249176502000000, 249176503000000),
  (249176504000000, 249176508000000),
  (249176512000000, 249176513000000),
  (249176518000000, 249176519000000),
  (249176521000000, 249176523000000),
  (249176525000000, 249176527000000),
  (249176529000000, 249176530000000),
  (249176541000000, 249176542000000),
  (249176549000000, 249176551000000),
  (249176552000000, 249176553000000),
  (249176556000000, 249176557000000),
  (249176560000000, 249176561000000),
  (249176562000000, 249176564000000),
  (249176579000000, 249176580000000),
  (249176581000000, 249176582000000),
  (249176583000000, 249176584000000),
  (249176588000000, 249176589000000),
  (249176590000000, 249176591000000),
  (249176592000000, 249176594000000),
  (249176598000000, 249176599000000),
  (249176602000000, 249176603000000),
  (249176605000000, 249176607000000),
  (249176611000000, 249176612000000),
  (249176617000000, 249176618000000),
  (249176620000000, 249176621000000),
  (249176624000000, 249176625000000),
  (249176626000000, 249176628000000),
  (249176634000000, 249176635000000),
  (249176639000000, 249176641000000),
  (249176642000000, 249176643000000),
  (249176645000000, 249176646000000),
  (249176650000000, 249176651000000),
  (249176654000000, 249176655000000),
  (249176660000000, 249176662000000),
  (249176667000000, 249176668000000),
  (249176671000000, 249176672000000),
  (249176673000000, 249176674000000),
  (249176676000000, 249176677000000),
  (249176678000000, 249176680000000),
  (249176686000000, 249176687000000),
  (249176690000000, 249176691000000),
  (249176694000000, 249176695000000),
  (249176696000000, 249176698000000),
  (249176900000000, 249180000000000),
  (249330000000000, 249332000000000),
  (249334000000000, 249334400000000),
  (249375000000000, 249380000000000),
  (249640002000000, 249640004000000),
  (249640008000000, 249640009000000),
  (249640010000000, 249640012000000),
  (249640014000000, 249640015000000),
  (249640021000000, 249640022000000),
  (249640024000000, 249640027000000),
  (249640029000000, 249640030000000),
  (249640032000000, 249640033000000),
  (249640035000000, 249640036000000),
  (249640038000000, 249640039000000),
  (249640041000000, 249640043000000),
  (249640047000000, 249640048000000),
  (249640051000000, 249640053000000),
  (249640061000000, 249640062000000),
  (249640065000000, 249640066000000),
  (249640067000000, 249640072000000),
  (249640076000000, 249640077000000),
  (249640080000000, 249640083000000),
  (249640088000000, 249640089000000),
  (249640090000000, 249640091000000),
  (249640093000000, 249640094000000),
  (249640096000000, 249640097000000),
  (249640098000000, 249640099000000),
  (249640100000000, 249640402000000),
  (249640404000000, 249640406000000),
  (249640411000000, 249640412000000),
  (249640413000000, 249640415000000),
  (249640418000000, 249640419000000),
  (249640420000000, 249640422000000),
  (249640432000000, 249640433000000),
  (249640434000000, 249640435000000),
  (249640436000000, 249640437000000),
  (249640444000000, 249640445000000),
  (249640449000000, 249640450000000),
  (249640454000000, 249640456000000),
  (249640458000000, 249640459000000),
  (249640467000000, 249640469000000),
  (249640476000000, 249640477000000),
  (249640478000000, 249640480000000),
  (249640485000000, 249640487000000),
  (249640488000000, 249640489000000),
  (249640492000000, 249640494000000),
  (249640498000000, 249640499000000),
  (249641000000000, 249641101000000),
  (249641102000000, 249641103000000),
  (249641106000000, 249641107000000),
  (249641111000000, 249641113000000),
  (249641119000000, 249641120000000),
  (249641124000000, 249641125000000),
  (249641128000000, 249641129000000),
  (249641133000000, 249641135000000),
  (249641140000000, 249641142000000),
  (249641145000000, 249641146000000),
  (249641149000000, 249641150000000),
  (249641151000000, 249641152000000),
  (249641153000000, 249641155000000),
  (249641157000000, 249641158000000),
  (249641164000000, 249641166000000),
  (249641167000000, 249641168000000),
  (249641170000000, 249641171000000),
  (249641172000000, 249641173000000),
  (249641175000000, 249641176000000),
  (249641180000000, 249641181000000),
  (249641182000000, 249641183000000),
  (249641184000000, 249641185000000),
  (249641186000000, 249641187000000),
  (249641193000000, 249641194000000),
  (249641196000000, 249641197000000),
  (249641199000000, 249641300000000),
  (249641303000000, 249641306000000),
  (249641309000000, 249641310000000),
  (249641316000000, 249641317000000),
  (249641320000000, 249641321000000),
  (249641327000000, 249641328000000),
  (249641329000000, 249641330000000),
  (249641331000000, 249641337000000),
  (249641344000000, 249641347000000),
  (249641350000000, 249641351000000),
  (249641352000000, 249641353000000),
  (249641357000000, 249641358000000),
  (249641359000000, 249641360000000),
  (249641362000000, 249641365000000),
  (249641370000000, 249641371000000),
  (249641373000000, 249641374000000),
  (249641375000000, 249641376000000),
  (249641377000000, 249641379000000),
  (249641381000000, 249641382000000),
  (249641389000000, 249641390000000),
  (249641391000000, 249641392000000),
  (249641395000000, 249641397000000),
  (249641399000000, 249642200000000),
  (249643700000000, 249644029000000),
  (249644200000000, 249645000000000),
  (249930000000000, 249934995000000),
  (250100000000000, 250105000000000),
  (250785000000000, 250790000000000),
  (250990000000000, 250995000000000),
  (251050000000000, 251055000000000),
  (251090006000000, 251090008000000),
  (251090011000000, 251090012000000),
  (251090014000000, 251090015000000),
  (251090017000000, 251090018000000),
  (251090020000000, 251090021000000),
  (251090025000000, 251090026000000),
  (251090028000000, 251090029000000),
  (251090036000000, 251090037000000),
  (251090041000000, 251090042000000),
  (251090044000000, 251090046000000),
  (251090051000000, 251090052000000),
  (251090054000000, 251090056000000),
  (251090061000000, 251090062000000),
  (251090069000000, 251090073000000),
  (251090080000000, 251090081000000),
  (251090083000000, 251090084000000),
  (251090095000000, 251090096000000),
  (251090097000000, 251090099000000),
  (251090101000000, 251090103000000),
  (251090104000000, 251090105000000),
  (251090108000000, 251090110000000),
  (251090113000000, 251090114000000),
  (251090115000000, 251090116000000),
  (251090118000000, 251090120000000),
  (251090122000000, 251090123000000),
  (251090124000000, 251090125000000),
  (251090132000000, 251090133000000),
  (251090138000000, 251090139000000),
  (251090144000000, 251090145000000),
  (251090150000000, 251090151000000),
  (251090155000000, 251090157000000),
  (251090163000000, 251090164000000),
  (251090167000000, 251090168000000),
  (251090169000000, 251090170000000),
  (251090172000000, 251090173000000),
  (251090174000000, 251090175000000),
  (251090176000000, 251090177000000),
  (251090178000000, 251090179000000),
  (251090183000000, 251090184000000),
  (251090189000000, 251090193000000),
  (251090194000000, 251090197000000),
  (251090205000000, 251090206000000),
  (251090209000000, 251090211000000),
  (251090217000000, 251090218000000),
  (251090219000000, 251090220000000),
  (251090221000000, 251090223000000),
  (251090225000000, 251090226000000),
  (251090229000000, 251090231000000),
  (251090234000000, 251090236000000),
  (251090243000000, 251090244000000),
  (251090246000000, 251090247000000),
  (251090248000000, 251090249000000),
  (251090256000000, 251090257000000),
  (251090258000000, 251090259000000),
  (251090264000000, 251090265000000),
  (251090268000000, 251090270000000),
  (251090275000000, 251090276000000),
  (251090285000000, 251090286000000),
  (251090288000000, 251090290000000),
  (251090300000000, 251090301000000),
  (251090303000000, 251090304000000),
  (251090305000000, 251090306000000),
  (251090308000000, 251090309000000),
  (251090310000000, 251090311000000),
  (251090320000000, 251090321000000),
  (251090326000000, 251090327000000),
  (251090328000000, 251090330000000),
  (251090334000000, 251090335000000),
  (251090341000000, 251090342000000),
  (251090351000000, 251090352000000),
  (251090355000000, 251090358000000),
  (251090359000000, 251090360000000),
  (251090363000000, 251090364000000),
  (251090369000000, 251090371000000),
  (251090374000000, 251090376000000),
  (251090381000000, 251090383000000),
  (251090386000000, 251090387000000),
  (251090391000000, 251090392000000),
  (251090394000000, 251090395000000),
  (251090396000000, 251090397000000),
  (251090398000000, 251095000000000),
  (251890000000000, 251895000000000),
  (252240000000000, 252245000000000),
  (252310000000000, 252315000000000),
  (252700000000000, 252705000000000),
  (253405000000000, 253410000000000),
  (253935000000000, 253940000000000),
  (254090000000000, 254095000000000),
  (254240000000000, 254245000000000),
  (254955000000000, 254960000000000),
  (255910000000000, 255915000000000),
  (255980000000000, 255985000000000),
  (256215000000000, 256220000000000),
  (256445000000000, 256450000000000),
  (256625000000000, 256630000000000),
  (256850000000000, 256855000000000),
  (256855000000000, 256860000000000),
  (257020000000000, 257025000000000),
  (257045001000000, 257045005000000),
  (257045015000000, 257045017000000),
  (257045018000000, 257045019000000),
  (257045022000000, 257045023000000),
  (257045026000000, 257045027000000),
  (257045030000000, 257045031000000),
  (257045033000000, 257045034000000),
  (257045041000000, 257045043000000),
  (257045044000000, 257045045000000),
  (257045051000000, 257045052000000),
  (257045054000000, 257045055000000),
  (257045058000000, 257045059000000),
  (257045063000000, 257045064000000),
  (257045067000000, 257045068000000),
  (257045072000000, 257045075000000),
  (257045080000000, 257045081000000),
  (257045083000000, 257045084000000),
  (257045087000000, 257045088000000),
  (257045090000000, 257050000000000),
  (257470000000000, 257475000000000),
  (257485000000000, 257490000000000),
  (257700000000000, 257705000000000),
  (258030000000000, 258035000000000),
  (258395000000000, 258395100000000),
  (258715000000000, 258720000000000),
  (258740000000000, 258745000000000),
  (258970000000000, 258975000000000),
  (259010000000000, 259015000000000),
  (259280000000000, 259285000000000),
  (260575000000000, 260580000000000),
  (260580000000000, 260585000000000),
  (260620000000000, 260625000000000),
  (260764000000000, 260765000000000),
  (261560000000000, 261565000000000),
  (261700000000000, 261705000000000),
  (262005000000000, 262010000000000),
  (263080000000000, 263081500000000),
  (263084000000000, 263084500000000),
  (265675000000000, 265680000000000),
  (265685000000000, 265690000000000),
  (265915000000000, 265920000000000),
  (267295000000000, 267300000000000),
  (267390000000000, 267395000000000),
  (267560000000000, 267565000000000),
  (268250000000000, 268255000000000),
  (268335000000000, 268340000000000),
  (268390000000000, 268395000000000),
  (268675000000000, 268680000000000),
  (269075000000000, 269080000000000),
  (271725000000000, 271730000000000),
  (272150000000000, 272155000000000),
  (272155000000000, 272160000000000),
  (272195000000000, 272200000000000),
  (272200000000000, 272205000000000),
  (272505000000000, 272510000000000),
  (276210000000000, 276215000000000),
  (276495000000000, 276500000000000),
  (276955000000000, 276960000000000),
  (277100000000000, 277105000000000),
  (277185000000000, 277190000000000),
  (278660000000000, 278665000000000),
  (278715000000000, 278720000000000),
  (278720000000000, 278725000000000),
  (278830000000000, 278835000000000),
  (278980000000000, 278985000000000),
  (279080000000000, 279085000000000),
  (279140000000000, 279145000000000),
  (279350000000000, 279355000000000),
  (279360000000000, 279365000000000),
  (279405000000000, 279410000000000),
  (279465000000000, 279470000000000),
  (279495000000000, 279500000000000),
  (279510000000000, 279515000000000),
  (279880000000000, 279885000000000),
  (280070000000000, 280075000000000),
  (280075000000000, 280080000000000),
  (280090000000000, 280095000000000),
  (280095000000000, 280100000000000),
  (280140000000000, 280145000000000),
  (280150000000000, 280155000000000),
  (280160000000000, 280165000000000),
  (280170000000000, 280175000000000),
  (280265000000000, 280270000000000),
  (280290000000000, 280295000000000),
  (280305000000000, 280310000000000),
  (280330000000000, 280335000000000),
  (280345000000000, 280350000000000),
  (280385000000000, 280390000000000),
  (280390000000000, 280395000000000),
  (280420000000000, 280425000000000),
  (280445000000000, 280450000000000),
  (280480000000000, 280485000000000),
  (280520000000000, 280525000000000),
  (280555000000000, 280560000000000),
  (280585000000000, 280590000000000),
  (280625000000000, 280630000000000),
  (280630000000000, 280635000000000),
  (280635000000000, 280640000000000),
  (280680000000000, 280685000000000),
  (280750000000000, 280755000000000),
  (280760000000000, 280765000000000),
  (280810000000000, 280815000000000),
  (280820000000000, 280825000000000),
  (280835000000000, 280840000000000),
  (280840000000000, 280845000000000),
  (280845000000000, 280850000000000),
  (280850000000000, 280855000000000),
  (280910000000000, 280915000000000),
  (280915000000000, 280920000000000),
  (280930000000000, 280935000000000),
  (280940000000000, 280945000000000),
  (280945000000000, 280950000000000),
  (280980000000000, 280985000000000),
  (280995000000000, 281000000000000),
  (281005000000000, 281010000000000),
  (281070000000000, 281075000000000),
  (281095000000000, 281100000000000),
  (281100000000000, 281105000000000),
  (281740000000000, 281745000000000),
  (281755000000000, 281760000000000),
  (281770000000000, 281775000000000),
  (281780000000000, 281785000000000),
  (281805000000000, 281810000000000),
  (281840000000000, 281845000000000),
  (281860000000000, 281865000000000),
  (281870000000000, 281875000000000),
  (281910000000000, 281915000000000),
  (281970000000000, 281975000000000),
  (281995000000000, 282000000000000),
  (282000000000000, 282005000000000),
  (282030000000000, 282035000000000),
  (282035000000000, 282040000000000),
  (282060000000000, 282065000000000),
  (282065000000000, 282070000000000),
  (282520000000000, 282525000000000),
  (282535000000000, 282540000000000),
  (282560000000000, 282565000000000),
  (282605000000000, 282610000000000),
  (282655000000000, 282660000000000),
  (282665000000000, 282670000000000),
  (282695000000000, 282700000000000),
  (282730000000000, 282735000000000),
  (282755000000000, 282760000000000),
  (282775000000000, 282780000000000),
  (282815000000000, 282820000000000),
  (282885000000000, 282890000000000),
  (282945000000000, 282950000000000),
  (282960000000000, 282965000000000),
  (282985000000000, 282990000000000),
  (282990000000000, 282995000000000),
  (282995000000000, 283000000000000),
  (283620000000000, 283625000000000),
  (283740000000000, 283745000000000),
  (283755000000000, 283760000000000),
  (283840000000000, 283845000000000),
  (283885000000000, 283889901000000),
  (283915000000000, 283920000000000),
  (283930000000000, 283935000000000),
  (283935000000000, 283940000000000),
];
