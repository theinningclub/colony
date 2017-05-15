#![recursion_limit = "1024"]
#![feature(custom_attribute)]
#![feature(try_from)]

extern crate dotenv;
#[macro_use] extern crate error_chain;
extern crate hyper;
extern crate itertools;
extern crate select;
extern crate serde;
#[macro_use] extern crate serde_derive;
extern crate serde_json;

use std::convert::TryFrom;
use std::convert::TryInto;
use std::io::Write;

use serde_json::to_string;

use hyper::client::Client;
use hyper::client::Response;

use itertools::Itertools;

use select::document::Document;
use select::predicate::{Predicate, Attr, Class, Name};


error_chain!{
  foreign_links {
    IO(::std::io::Error);
    HTTP(::hyper::Error);
    JSON(::serde_json::Error);
  }

  errors {
      RemoteError(t: String) {
          description("corporation database error")
          display("corporation database error: \"{}\"", t)
      }
      SelectionError(f: String) {
          description("html selection error")
          display("could not select: \"{}\"", f)
      }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct Corporation {
  id                : u32,
  name              : String,
  kind              : String,
  purpose           : String,
  date_incorporated : String,
  date_effective    : String,
  principal_office  : PrincipalOffice,
  principal_agent   : PrincipalAgent,
  officers          : Vec<Officer>,
  renames           : Vec<Rename>,
  merged_from       : Vec<Merger>,
  merged_into       : Vec<Merger>
}

#[derive(Serialize, Deserialize, Debug)]
struct PrincipalOffice {
  address           : Address,
  maintained        : String
}


#[derive(Serialize, Deserialize, Debug)]
struct PrincipalAgent {
  name              : String,
  address           : Address,
  resigned          : String
}

#[derive(Serialize, Deserialize, Debug)]
struct Address {
  street            : String,
  city              : String,
  state             : String,
  zip               : String,
  country           : String,
}

#[derive(Serialize, Deserialize, Debug)]
struct Officer {
  title             : String,
  name              : String,
  address           : String,
}

#[derive(Serialize, Deserialize, Debug)]
struct Merger {
  corp              : String,
  date              : String,
}

#[derive(Serialize, Deserialize, Debug)]
struct Rename {
  name              : String,
  date              : String,
}


macro_rules! select {
  ($document:expr, $id:expr) => (
    try!($document.find(Attr("id",  $id))
      .next().map(|node| node.text().trim().into())
      .ok_or(ErrorKind::SelectionError($id.into())))
  )
}


impl<'t> TryFrom<(u32, &'t Document)> for Corporation {
  type Error = Error;
  fn try_from((id, document): (u32, &Document)) -> Result<Self> {
    Ok(Corporation {
      id                : id,
      name              : select!(document, "MainContent_lblEntityName"),
      kind              : select!(document, "MainContent_lblEntityType"),
      purpose           : select!(document, "MainContent_txtComments"),
      date_incorporated : select!(document, "MainContent_lblOrganisationDate"),
      date_effective    : select!(document, "MainContent_lblOrganisationDate"),
      principal_office  : try!(document.try_into()),
      principal_agent   : try!(document.try_into()),
      officers          :
        document
          .find(
            Attr("id", "MainContent_grdOfficers")
              .descendant(Class("GridRow"))
                .descendant(Name("td")))
          .map(|node| node.text())
          .tuples()
          .map(|(title, name, address)|
            Officer {
              title     : title.trim().into(),
              name      : name.trim().into(),
              address   : address.trim().into()
            }).collect_vec(),
      renames           :
        try!(document
          .find(
            Attr("id", "MainContent_tblNameChange")
              .descendant(Name("div").and(Class("p1"))))
          .map(|node|
            { let mut children = node.children();
              let name = children.nth(1)
                .ok_or("could not parse rename name")?
                .text().trim().into();
              let date =
                  children.nth(1)
                    .map(|node|node.text())
                    .ok_or(ErrorKind::SelectionError("could not select rename date".into()))?
                .trim().into();
              Ok(Rename {
                name: name,
                date: date
              })
            }).collect::<Result<Vec<Rename>>>()),
      merged_from       :
        try!(document
          .find(
            Attr("id", "MainContent_tblMergedWith")
              .descendant(Name("tr"))
                .descendant(Name("td"))
                  .descendant(Class("p1")))
          .map(|node|
            { let mut children = node.children();
              let corp = String::from(children.nth(2)
                .and_then(|node| node.attr("href"))
                .and_then(|href| href.split('=').nth(1))
                .ok_or("could not parse merger name")?);
              let date =
                  children.nth(2)
                    .map(|node| node.text())
                    .ok_or(ErrorKind::SelectionError("could not select merger date".into()))?
                .trim().into();
              Ok(Merger {
                corp: corp,
                date: date
              })
            }).collect::<Result<Vec<Merger>>>()),
      merged_into       :
        try!(document
          .find(
            Attr("id", "MainContent_tblMergedInto")
              .descendant(Name("tr"))
                .descendant(Name("td"))
                  .descendant(Class("p1")))
          .map(|node|
            { let mut children = node.children();
              let corp = String::from(children.nth(2)
                .and_then(|node| node.attr("href"))
                .and_then(|href| href.split('=').nth(1))
                .ok_or("could not parse merger name")?);
              let date =
                  children.nth(2)
                    .map(|node| node.text())
                    .ok_or(ErrorKind::SelectionError("could not select merger date".into()))?
                .trim().into();
              Ok(Merger {
                corp: corp,
                date: date
              })
            }).collect::<Result<Vec<Merger>>>())
      })
  }
}


impl<'t> TryFrom<&'t Document> for PrincipalOffice {
  type Error = Error;
  fn try_from(document: &Document) -> Result<Self> {
    Ok(PrincipalOffice {
      address           :
        Address {   
          street        : select!(document, "MainContent_lblPrincipleStreet"),
          city          : select!(document, "MainContent_lblPrincipleCity"),
          state         : select!(document, "MainContent_lblPrincipleState"),
          zip           : select!(document, "MainContent_lblPrincipleZip"),
          country       : select!(document, "MainContent_lblPrincipleCountry"),
        },    
      maintained        : select!(document, "MainContent_lblConsentFlag")})
  }
}


impl<'t> TryFrom<&'t Document> for PrincipalAgent {
  type Error = Error;
  fn try_from(document: &Document) -> Result<Self> {
    Ok(PrincipalAgent {
      name              : select!(document, "MainContent_lblResidentAgentName"),
      address           :
        Address {   
          street        : select!(document, "MainContent_lblResidentStreet"),
          city          : select!(document, "MainContent_lblResidentCity"),
          state         : select!(document, "MainContent_lblResidentState"),
          zip           : select!(document, "MainContent_lblResidentZip"),
          country       : select!(document, "MainContent_lblResidentCountry"),
        },
      resigned          : select!(document, "MainContent_lblResidentAgentFlag")})
  }
}


fn fetch(client: &Client, id: u32) -> Result<Response> {
  client
    .get(&format!("http://business.sos.ri.gov/CorpWeb/CorpSearch/CorpSummary.aspx?FEIN={:09}", id))
    .send()
    .chain_err(|| format!("Could not fetch corporation #{:09}", id))
}


fn select(id: u32, document: &Document) -> Result<Corporation>
{
  if let Some(error_message) = document.find(Class("ErrorMessage")).next()
  {
    bail!(ErrorKind::RemoteError(error_message.text()))
  }
  (id, document).try_into()
}

fn run(client: &Client, id: u32) -> Result<Corporation> {
  let raw = fetch(client, id)?;
  let doc = Document::from_read(raw)?;
  select(id, &doc)
}

fn main() {
  let client = Client::new();

  let mut stdout = std::io::stdout();
  let mut stdout = stdout.lock();
  let mut stderr = std::io::stderr();
  let mut stderr = stderr.lock();

  //let range = (0..1000000);
  let range = (0..100);

  let results = range.clone().zip(
    range.clone()
      .map(|id| run(&client, id))
      .map(|result|
        result.and_then(|corp|
          to_string(&corp)
            .chain_err(|| format!("could not serialize")))));

  for (id, result) in results {
    match result {
      Ok(result) => {writeln!(stdout, "{}", result);},
      Err(ref e) => {
        writeln!(stderr, "error on {}: {}", id, e);
        for e in e.iter().skip(1) {
          writeln!(stderr, "caused by: {}", e);
        }
        if let Some(backtrace) = e.backtrace() {
          writeln!(stderr, "backtrace: {:?}", backtrace);
        }

        if let &Error(ErrorKind::RemoteError(_), _) = e {
          continue;
        } else {::std::process::exit(1);}
      }
    }
  }
}
