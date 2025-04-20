use url::Url;

pub fn join_endpoint(base: Url, query: &str) -> Result<Url, url::ParseError> {
    let mut res = base.join(query)?;
    res.set_scheme(base.scheme()).unwrap();
    res.set_username(base.username()).unwrap();
    res.set_password(base.password()).unwrap();
    res.set_host(base.host_str())?;
    res.set_port(base.port()).unwrap();
    Ok(res)
}
