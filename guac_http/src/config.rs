#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(dead_code)]

use std::borrow::Cow;

#[derive(Debug, Clone)]
#[allow(non_camel_case_types)]
pub struct Config {
    pub contract_address: Cow<'static, str>,
    pub private_key_0: Cow<'static, str>,
    pub private_key_1: Cow<'static, str>,
}

pub const CONFIG: Config = Config {
    contract_address: Cow::Borrowed("0x378899c6d5e76716c732a9e774b862514e874d99 shibby"),
    private_key_0: Cow::Borrowed("86de2cf259bf21a9aa2b8cf78f89ed479681001ca320c5762bb3237db65445cb"),
    private_key_1: Cow::Borrowed("06e744bba37fd1e630dc775d10fd8cbe0b5643f4d7187072d3d08df4b4118acf"),
};
