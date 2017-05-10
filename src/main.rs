#![recursion_limit = "1024"]
#![feature(try_from)] 

extern crate chrono;
#[macro_use]
extern crate error_chain;

extern crate hyper;
extern crate itertools;

extern crate select;

use std::convert::TryFrom;
use std::convert::TryInto;

use chrono::NaiveDate;

use hyper::client::Client;
use hyper::client::Response;

use itertools::Itertools;

use select::document::Document;
use select::predicate::{Predicate, Attr, Class, Name};


error_chain!{
  foreign_links {
    IO(::std::io::Error);
    HTTP(::hyper::Error);
    Parse(::chrono::ParseError);
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


#[derive(Debug)]
struct Corporation {
  name              : String,
  kind              : String,
  purpose           : String,
  date_incorporated : NaiveDate,
  date_effective    : NaiveDate,
  principal_office  : PrincipalOffice,
  principal_agent   : PrincipalAgent,
  officers          : Vec<Officer>,
  renames           : Vec<Rename>,
  merged_from       : Vec<Merger>,
  merged_into       : Vec<Merger>
}


#[derive(Debug)]
struct PrincipalOffice {
  address           : Address,
  maintained        : String
}


#[derive(Debug)]
struct PrincipalAgent {
  name              : String,
  address           : Address,
  resigned          : String
}


#[derive(Debug)]
struct Address {
  street            : String,
  city              : String,
  state             : String,
  zip               : String,
  country           : String,
}


#[derive(Debug)]
struct Officer {
  title             : String,
  name              : String,
  address           : String,
}


#[derive(Debug)]
struct Merger {
  corp             : String,
  date             : NaiveDate,
}


#[derive(Debug)]
struct Rename {
  name             : String,
  date             : NaiveDate,
}


macro_rules! select_text {
  ($document:expr, $id:expr) => (
    try!($document.find(Attr("id",  $id))
      .next().map(|node| node.text().trim().into())
      .ok_or(ErrorKind::SelectionError($id.into())))
  )
}


macro_rules! select_date {
  ($document:expr, $id:expr, $format:expr) => (
    try!(NaiveDate::parse_from_str(
      try!($document.find(Attr("id", $id))
      .next().map(|node|node.text())
      .ok_or(ErrorKind::SelectionError($id.into()))).trim(), $format))
  )
}


impl<'t> TryFrom<&'t Document> for Corporation {
  type Error = Error;
  fn try_from(document: &Document) -> Result<Self> {
    Ok(Corporation {
      name              : select_text!(document, "MainContent_lblEntityName"),
      kind              : select_text!(document, "MainContent_lblEntityType"),
      purpose           : select_text!(document, "MainContent_txtComments"),
      date_incorporated : select_date!(document, "MainContent_lblOrganisationDate", "%m-%d-%Y"),
      date_effective    : select_date!(document, "MainContent_lblOrganisationDate", "%m-%d-%Y"),
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
                NaiveDate::parse_from_str(
                  children.nth(1)
                    .map(|node|node.text())
                    .ok_or(ErrorKind::SelectionError("could not select rename date".into()))?
                .trim(), "%m-%d-%Y")?;
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
                NaiveDate::parse_from_str(
                  children.nth(2)
                    .map(|node| node.text())
                    .ok_or(ErrorKind::SelectionError("could not select merger date".into()))?
                .trim(), "%m-%d-%Y")?;
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
                NaiveDate::parse_from_str(
                  children.nth(2)
                    .map(|node| node.text())
                    .ok_or(ErrorKind::SelectionError("could not select merger date".into()))?
                .trim(), "%m-%d-%Y")?;
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
          street        : select_text!(document, "MainContent_lblPrincipleStreet"),
          city          : select_text!(document, "MainContent_lblPrincipleCity"),
          state         : select_text!(document, "MainContent_lblPrincipleState"),
          zip           : select_text!(document, "MainContent_lblPrincipleZip"),
          country       : select_text!(document, "MainContent_lblPrincipleCountry"),
        },    
      maintained        : select_text!(document, "MainContent_lblConsentFlag")})
  }
}


impl<'t> TryFrom<&'t Document> for PrincipalAgent {
  type Error = Error;
  fn try_from(document: &Document) -> Result<Self> {
    Ok(PrincipalAgent {
      name              : select_text!(document, "MainContent_lblResidentAgentName"),
      address           :
        Address {   
          street        : select_text!(document, "MainContent_lblResidentStreet"),
          city          : select_text!(document, "MainContent_lblResidentCity"),
          state         : select_text!(document, "MainContent_lblResidentState"),
          zip           : select_text!(document, "MainContent_lblResidentZip"),
          country       : select_text!(document, "MainContent_lblResidentCountry"),
        },    
      resigned          : select_text!(document, "MainContent_lblResidentAgentFlag")})
  }
}


fn fetch(client: &Client, id: u32) -> Result<Response> {
  client
    .get(&format!("http://business.sos.ri.gov/CorpWeb/CorpSearch/CorpSummary.aspx?FEIN={:09}", id))
    .send()
    .chain_err(|| format!("Could not fetch corporation #{:09}", id))
}


fn select(document: &Document) -> Result<Corporation>
{
  if let Some(error_message) = document.find(Class("ErrorMessage")).next()
  {
    bail!(ErrorKind::RemoteError(error_message.text()))
  }
  document.try_into()
}


fn run() -> Result<()> {
  let client = Client::new();
  let result = try!(fetch(&client, 000963237));
  let result = try!(Document::from_read(result));
  let result = try!(select(&result));
  println!("{:?}", result);
  Ok(())
}


fn main() {
  if let Err(ref e) = run() {
    println!("error: {}", e);
    for e in e.iter().skip(2) {
      println!("caused by: {}", e);
    }
    if let Some(backtrace) = e.backtrace() {
      println!("backtrace: {:?}", backtrace);
    }
    std::process::exit(1);
  }
}
