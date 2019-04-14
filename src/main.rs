#![allow(unused_variables)]

extern crate actix;
extern crate actix_web;
extern crate env_logger;
extern crate futures;
extern crate regex;
extern crate serde_json;
extern crate maxminddb;
#[macro_use]
extern crate tera;

use std::env;
use std::net::IpAddr;
use std::str::FromStr;
use std::collections::{HashMap, BTreeMap};

use regex::Regex;
use maxminddb::geoip2::Country;
use actix_web::http::{Method};
use actix_web::{
    fs, error, middleware, server, App, Error, Query, State,
    HttpRequest, HttpResponse, Result
};

struct AppState {
    template: tera::Tera,
}

fn is_cli(req: &HttpRequest<AppState>) -> bool {
    let user_agent = format!("{:?}", req.headers().get("user-agent").unwrap());
    let re = Regex::new(r"(curl|wget|Wget|fetch slibfetch)/.*$").unwrap();
    return re.is_match(&user_agent)
}

fn lookup_ip(req: &HttpRequest<AppState>) -> String {
    return format!("{}", req.connection_info()
                            .remote()
                            .unwrap()
                            .splitn(2, ":")
                            .next()
                            .unwrap()
                        )
}

fn lookup_cmd(cmd: &str) -> &str {
    let s = match cmd {
        "curl"  => "curl",
        "wget"  => "wget -qO -",
        "fetch" => "fetch -qo -",
        _       => ""
    };
    return s
}

fn lookup_country(ip_address: &String) -> Option<BTreeMap<String, String>> {
    // Lookup Country from user's public ip address. The GeoLite2-Country.mmdb
    // can be downloaded from https://dev.maxmind.com/geoip/geoip2/geolite2
    let reader = maxminddb::Reader::open("GeoLite2-Country.mmdb").unwrap();
    let ip: IpAddr = FromStr::from_str(ip_address).unwrap();
    match reader.lookup(ip) {
        Ok(db)      => {
            let db : Country = db;
            return db.country.and_then(|n| n.names)
            }
        Err(error)  => {
            println!("Error during looking up ip {:?} the DB: {:?}", ip_address, error);
            let mut default_country = BTreeMap::new();
            default_country.insert(String::from("en"), String::from("Unknown"));
            return Some(default_country)
            }
    };
}

fn render_template(state: State<AppState>, template: &str) -> Result<HttpResponse, Error> {
    let s = state
            .template
            .render(template, &tera::Context::new())
            .map_err(|_| error::ErrorInternalServerError("Template error"))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(s))
}

fn favicon(state: State<AppState>) -> Result<fs::NamedFile> {
    Ok(fs::NamedFile::open("static/favicon.ico")?)
}

fn p404(state: State<AppState>) -> Result<HttpResponse, Error> {
    render_template(state, "404.html")
}

fn index(req: HttpRequest<AppState>,
         query: Query<HashMap<String, String>>) -> Result<HttpResponse, Error> {
    let ip_address = lookup_ip(&req);
    // If user browses the index page by using command line,
    // return the public ip address instead of whole html page.
    if is_cli(&req) {
        return Ok(HttpResponse::Ok().content_type("text/plain").body(ip_address + "\n"))
    }

    let default_cmd = String::from("curl");
    let cmd = query.get("cmd").unwrap_or(&default_cmd);
    let country = lookup_country(&ip_address);
    let country_name =  match &country {
                                None => "Unknown",
                                Some (n) => n.get("en").unwrap()
                            };

    // Create a HashMap from request's header
    // for later using with json.
    let mut headers = HashMap::new();
    for (key, value) in req.headers().iter() {
        let k = format!("{:}", key);
        let mut v = format!("{:?}", value);
        v = v.replace('"', "");
        headers.insert(k, v);
    }

    // Remove Host header
    headers.remove("host");
    // Add country and ip address into above HashMap
    headers.insert(String::from("country"), format!("{:}", country_name));
    headers.insert(String::from("ip-address"), format!("{:}", ip_address));

    // Create a Context for template variable rendering
    let mut context = tera::Context::new();
    context.insert("cmd", &cmd);
    context.insert("cmd_with_options", lookup_cmd(&cmd));
    context.insert("ip_address", &ip_address);
    context.insert("headers", &headers);
    context.insert("country", country_name);

    let rendered = req.state()
        .template
        .render("index.html", &context).map_err(|e| {
            error::ErrorInternalServerError(e.description().to_owned())
        })?;

    Ok(HttpResponse::Ok().content_type("text/html").body(rendered))
}

fn custom_query(req: HttpRequest<AppState>,
                state: State<AppState>,
                query: Query<HashMap<String, String>>) -> Result<HttpResponse, Error> {
    let param = req.match_info().get("param").unwrap();
    match param {
        "country" => {
                let ip_address = lookup_ip(&req);
                let country = lookup_country(&ip_address);
                match country {
                    Some (n) => return Ok(HttpResponse::Ok()
                                        .content_type("text/plain")
                                        .body(n.get("en").unwrap())
                                        ),
                    _ => return Ok(HttpResponse::Ok()
                                .content_type("text/plain")
                                .body("Unknown"))
                }
            }
        "all.json" => {
                let mut headers = HashMap::new();
                for (key, value) in req.headers().iter() {
                    let k = format!("{:}", key);
                    let mut v = format!("{:?}", value);
                    v = v.replace('"', "");
                    headers.insert(k, v);
                }

                let ip_address = lookup_ip(&req);
                let country = lookup_country(&ip_address);
                let country_name =  match &country {
                                None => "Unknown",
                                Some (n) => n.get("en").unwrap()
                            };

                headers.remove("host");
                headers.insert(String::from("country"), format!("{:}", country_name));
                headers.insert(String::from("ip-address"), format!("{:}", ip_address));
                let result = serde_json::to_string_pretty(&headers).unwrap();
                return Ok(HttpResponse::Ok().content_type("application/json").body(result))
        }
        _   => {
                if req.headers().contains_key(param) {
                    let output = format!("{:?}", req.headers().get(param).unwrap());
                    return Ok(HttpResponse::Ok().content_type("text/plain").body(output))
                } else {
                    if is_cli(&req) {
                        return Ok(HttpResponse::Ok().content_type("text/html").body(""))
                    }
                    render_template(state, "404.html")
                }
        }
    }
}

fn main() {
    env::set_var("RUST_LOG", "actix_web=debug");
    env::set_var("RUST_BACKTRACE", "1");
    env_logger::init();
    let sys = actix::System::new("ifconfig.top");

    let addr = server::new(|| {
        let tera = compile_templates!(concat!(env!("CARGO_MANIFEST_DIR"), "/templates/**/*"));
        App::with_state(AppState{template: tera})
            // enable logger
            .middleware(middleware::Logger::default())
            // register favicon
            .resource("/favicon.ico", |r| r.method(Method::GET).with(favicon))
            // register home page
            .resource("/", |r| r.method(Method::GET).with(index))
            // register custom queries
            .resource("/{param}", |r| r.method(Method::GET).with(custom_query))
            // default page
            .default_resource(|r| r.method(Method::GET).with(p404))
            })
        .bind("0.0.0.0:9292").expect("Can not bind to 0.0.0.0:9292")
        .shutdown_timeout(0)
        .start();

    println!("Starting web server: 0.0.0.0:9292");
    let _ = sys.run();
}
