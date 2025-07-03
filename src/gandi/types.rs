
use serde::{Deserialize, Serialize};

// See https://api.gandi.net/docs/livedns/

// [
//   {
//     "rrset_name": "@",
//     "rrset_ttl": 10800,
//     "rrset_type": "A",
//     "rrset_values": [
//       "192.0.2.1"
//     ],
//     "rrset_href": "https://api.test/v5/livedns/domains/example.com/records/%40/A"
//   },
// ]
#[derive(Serialize, Deserialize, Debug)]
pub struct Record {
    rrset_name: String,
    rrset_type: String,
    rrset_values: Vec<String>,
    rrset_href: String,
    rrset_ttl: Option<u32>,
}
