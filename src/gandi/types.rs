
use serde::{Deserialize, Serialize};

// See https://api.gandi.net/docs/livedns/

// {
//   "object": "HTTPNotFound",
//   "cause": "Not Found",
//   "code": 404,
//   "message": "The resource could not be found."
// }
#[derive(Serialize, Deserialize, Debug)]
pub struct Error {
    pub object: String,
    pub cause: String,
    pub code: u32,
    pub message: String,
}

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
    pub rrset_name: String,
    pub rrset_type: String,
    pub rrset_values: Vec<String>,
    pub rrset_href: String,
    pub rrset_ttl: Option<u32>,
}

// {
//   "rrset_values": [
//     "www.example.org"
//   ],
//   "rrset_ttl": 320
// }
#[derive(Serialize, Deserialize, Debug)]
pub struct RecordUpdate {
    pub rrset_values: Vec<String>,
    pub rrset_ttl: Option<u32>,
}

// {
//   "fqdn": "example.com",
//   "duration": 5,
//   "owner": {
//     "city": "Paris",
//     "given": "Alice",
//     "family": "Doe",
//     "zip": "75001",
//     "country": "FR",
//     "streetaddr": "5 rue neuve",
//     "phone": "+33.123456789",
//     "state": "FR-IDF",
//     "type": "individual",
//     "email": "alice@example.org"
//   }
// }
#[derive(Serialize, Deserialize, Debug)]
pub struct Owner {
    pub city: String,
    pub given: String,
    pub family: String,
    pub zip: String,
    pub country: String,
    pub streetaddr: String,
    pub phone: String,
    pub state: String,
    #[serde(rename = "type")] 
    pub owner_type: String,
    pub email: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CreateDomain {
    pub fqdn: String,
    pub owner: Owner,
}
