//! Built-in Windows strategies and the machine-generated catalogue used by
//! autoselection.

use crate::model::{Engine, Source, Strategy};
use crate::util::hash_id;

fn strategy(id: &str, engine: Engine, name: &str, template: String, tags: &[&str]) -> Strategy {
    let mut s = Strategy::new(id, engine, name, template);
    s.source = Source::Builtin;
    s.tags = tags.iter().map(|t| t.to_string()).collect();
    s
}

fn z2(tls: &str) -> String {
    format!(
        "--wf-tcp-out=80,443 --wf-udp-out=443 \
--filter-tcp=80 --filter-l7=http --hostlist-exclude=$EXCLUDE --payload=http_req \
--lua-desync=fake:blob=fake_default_http:tcp_md5 --lua-desync=multisplit:pos=method+2 \
--new \
--filter-tcp=443 --filter-l7=tls --hostlist-exclude=$EXCLUDE --payload=tls_client_hello {tls} \
--new \
--filter-udp=443 --filter-l7=quic --hostlist-exclude=$EXCLUDE --payload=quic_initial \
--lua-desync=fake:blob=fake_default_quic:repeats=6"
    )
}

fn z1(tls: &str) -> String {
    format!(
        "--wf-tcp=80,443 --wf-udp=443 \
--filter-tcp=80 --filter-l7=http --hostlist-exclude=$EXCLUDE \
--dpi-desync=fake,multisplit --dpi-desync-split-pos=method+2 --dpi-desync-fooling=md5sig \
--new \
--filter-tcp=443 --filter-l7=tls --hostlist-exclude=$EXCLUDE {tls} \
--new \
--filter-udp=443 --filter-l7=quic --hostlist-exclude=$EXCLUDE \
--dpi-desync=fake --dpi-desync-repeats=6"
    )
}

#[allow(clippy::vec_init_then_push)]
pub fn builtin() -> Vec<Strategy> {
    let mut out = Vec::new();

    // ---- Zapret 2
    out.push(strategy(
        "builtin-z2-faketls",
        Engine::Zapret2,
        "Zapret2 · FAKE TLS AUTO (универсальная)",
        z2("--lua-desync=fake:blob=fake_default_tls:tcp_md5:tls_mod=rnd,dupsid,sni=www.google.com:repeats=6 --lua-desync=multisplit:pos=1,midsld"),
        &["youtube", "discord", "general"],
    ));
    out.push(strategy(
        "builtin-z2-hostfakesplit",
        Engine::Zapret2,
        "Zapret2 · hostfakesplit (Google SNI)",
        z2("--lua-desync=hostfakesplit:host=www.google.com:tcp_ts=-600000:repeats=4"),
        &["youtube", "discord"],
    ));
    out.push(strategy(
        "builtin-z2-multidisorder",
        Engine::Zapret2,
        "Zapret2 · fake + multidisorder",
        z2("--lua-desync=fake:blob=fake_default_tls:tcp_md5:tcp_seq=-10000 --lua-desync=multidisorder:pos=1,midsld"),
        &["youtube", "discord"],
    ));
    out.push(strategy(
        "builtin-z2-multisplit",
        Engine::Zapret2,
        "Zapret2 · multisplit + seqovl",
        z2("--lua-desync=multisplit:pos=10:seqovl=1"),
        &["general"],
    ));
    out.push(strategy(
        "builtin-z2-fake-simple",
        Engine::Zapret2,
        "Zapret2 · простой fake TLS",
        z2("--lua-desync=fake:blob=fake_default_tls:tcp_md5"),
        &["general"],
    ));
    out.push(strategy(
        "builtin-z2-fake-badsum",
        Engine::Zapret2,
        "Zapret2 · fake TLS (badsum)",
        z2("--lua-desync=fake:blob=fake_default_tls:badsum"),
        &["general"],
    ));
    out.push(strategy(
        "builtin-z2-ts",
        Engine::Zapret2,
        "Zapret2 · fake TLS + timestamp",
        z2("--lua-desync=fake:blob=fake_default_tls:tcp_ts=-1000 --lua-desync=multidisorder:pos=2"),
        &["youtube"],
    ));

    // ---- legacy Zapret (winws.exe)
    out.push(strategy(
        "builtin-z1-fake-multisplit",
        Engine::Zapret,
        "Zapret · fake + multisplit",
        z1("--dpi-desync=fake,multisplit --dpi-desync-split-pos=1,midsld --dpi-desync-fooling=md5sig --dpi-desync-repeats=6"),
        &["youtube", "discord", "general"],
    ));
    out.push(strategy(
        "builtin-z1-multidisorder",
        Engine::Zapret,
        "Zapret · multidisorder",
        z1("--dpi-desync=multidisorder --dpi-desync-split-pos=1,midsld"),
        &["general"],
    ));
    out.push(strategy(
        "builtin-z1-fake-fakedsplit",
        Engine::Zapret,
        "Zapret · fake + fakedsplit",
        z1("--dpi-desync=fake,fakedsplit --dpi-desync-split-pos=1 --dpi-desync-fooling=md5sig"),
        &["youtube"],
    ));
    out.push(strategy(
        "builtin-z1-hostfakesplit",
        Engine::Zapret,
        "Zapret · hostfakesplit",
        z1("--dpi-desync=hostfakesplit --dpi-desync-fooling=md5sig"),
        &["discord"],
    ));
    out.push(strategy(
        "builtin-z1-fake-tls-bin",
        Engine::Zapret,
        "Zapret · fake TLS из файла",
        z1("--dpi-desync=fake,multisplit --dpi-desync-split-pos=1 --dpi-desync-fooling=md5sig --dpi-desync-fake-tls=$FAKE/tls_clienthello_www_google_com.bin --dpi-desync-repeats=6"),
        &["youtube"],
    ));

    out
}

const Z2_QUICK: &[&str] = &[
    "--lua-desync=multisplit:pos=1",
    "--lua-desync=multisplit:pos=2",
    "--lua-desync=multisplit:pos=1,midsld",
    "--lua-desync=multidisorder:pos=1,midsld",
    "--lua-desync=multisplit:pos=sniext+1",
    "--lua-desync=multisplit:pos=10:seqovl=1",
    "--lua-desync=tcpseg:pos=0,-1:seqovl=1 --lua-desync=drop",
    "--lua-desync=multisplit:pos=1:seqovl=#fake_default_tls:seqovl_pattern=fake_default_tls",
    "--lua-desync=fake:blob=fake_default_tls:tcp_md5",
    "--lua-desync=fake:blob=fake_default_tls:badsum",
    "--lua-desync=fake:blob=fake_default_tls:tcp_seq=-3000",
    "--lua-desync=fake:blob=fake_default_tls:ip_autottl=-2,3-20",
    "--lua-desync=fake:blob=fake_default_tls:tcp_md5 --lua-desync=multisplit:pos=1,midsld",
    "--lua-desync=fake:blob=fake_default_tls:tcp_md5:tcp_seq=-10000 --lua-desync=multidisorder:pos=1,midsld",
    "--lua-desync=fake:blob=fake_default_tls:tcp_md5:tls_mod=rnd,dupsid --lua-desync=multisplit:pos=2",
    "--lua-desync=fake:blob=fake_default_tls:tcp_ts=-1000 --lua-desync=multidisorder:pos=2",
    "--lua-desync=hostfakesplit:host=www.google.com:tcp_ts=-600000:repeats=4",
    "--lua-desync=hostfakesplit:host=www.google.com:tcp_ts=-600000:repeats=4:ip_id=zero",
    "--lua-desync=hostfakesplit:host=www.google.com:tcp_md5:repeats=4",
    "--lua-desync=fake:blob=fake_default_tls:tls_mod=rnd,dupsid,sni=www.google.com:tcp_ts=-1000:repeats=6 --lua-desync=multisplit:pos=1",
];

const Z1_QUICK: &[&str] = &[
    "--dpi-desync=multisplit --dpi-desync-split-pos=1",
    "--dpi-desync=multisplit --dpi-desync-split-pos=2",
    "--dpi-desync=multisplit --dpi-desync-split-pos=1,midsld",
    "--dpi-desync=multidisorder --dpi-desync-split-pos=1,midsld",
    "--dpi-desync=multisplit --dpi-desync-split-seqovl=1 --dpi-desync-split-pos=sld+1",
    "--dpi-desync=fake --dpi-desync-fooling=md5sig",
    "--dpi-desync=fake --dpi-desync-fooling=badseq",
    "--dpi-desync=fake --dpi-desync-autottl",
    "--dpi-desync=fake,multisplit --dpi-desync-split-pos=1,midsld --dpi-desync-fooling=md5sig",
    "--dpi-desync=fake,multidisorder --dpi-desync-split-pos=1,midsld --dpi-desync-fooling=badseq",
    "--dpi-desync=fake,fakedsplit --dpi-desync-split-pos=1 --dpi-desync-fooling=md5sig",
    "--dpi-desync=fake,fakeddisorder --dpi-desync-split-pos=midsld --dpi-desync-fooling=badseq",
    "--dpi-desync=syndata,multisplit --dpi-desync-split-pos=1",
    "--dpi-desync=hostfakesplit --dpi-desync-fooling=md5sig",
    "--dpi-desync=fake --dpi-desync-fake-tls=$FAKE/tls_clienthello_www_google_com.bin --dpi-desync-fooling=md5sig --dpi-desync-repeats=6",
];

fn z2_full() -> Vec<String> {
    let mut out: Vec<String> = Z2_QUICK.iter().map(|s| s.to_string()).collect();
    let positions = [
        "2",
        "1",
        "sniext+1",
        "sniext+4",
        "host+1",
        "midsld",
        "1,midsld",
        "1,midsld,1220",
        "1,sniext+1,host+1,midsld-2,midsld,midsld+2,endhost-1",
    ];
    for mode in ["multisplit", "multidisorder"] {
        for p in positions {
            out.push(format!("--lua-desync={mode}:pos={p}"));
        }
    }
    for p in ["10", "10,sniext+1", "10,sniext+4", "10,midsld"] {
        out.push(format!("--lua-desync=multisplit:pos={p}:seqovl=1"));
    }
    for f in [
        "tcp_md5",
        "badsum",
        "tcp_seq=-3000",
        "tcp_seq=1000000",
        "tcp_ack=-66000:tcp_ts_up",
        "tcp_ts=-1000",
        "ip_autottl=-1,3-20",
        "ip_autottl=-2,3-20",
        "ip_ttl=4",
    ] {
        out.push(format!("--lua-desync=fake:blob=fake_default_tls:{f}"));
        out.push(format!(
            "--lua-desync=fake:blob=fake_default_tls:{f} --lua-desync=multisplit:pos=1,midsld"
        ));
        out.push(format!(
            "--lua-desync=fake:blob=fake_default_tls:{f} --lua-desync=multidisorder:pos=1,midsld"
        ));
        out.push(format!(
            "--lua-desync=fake:blob=fake_default_tls:{f}:tls_mod=rnd,dupsid,padencap"
        ));
    }
    out
}

fn z1_full() -> Vec<String> {
    let mut out: Vec<String> = Z1_QUICK.iter().map(|s| s.to_string()).collect();
    let positions = [
        "1",
        "2",
        "midsld",
        "1,midsld",
        "sniext+1",
        "host+1",
        "1,sniext+1,host+1,midsld",
    ];
    for mode in ["multisplit", "multidisorder"] {
        for p in positions {
            out.push(format!("--dpi-desync={mode} --dpi-desync-split-pos={p}"));
        }
    }
    for f in [
        "--dpi-desync-fooling=md5sig",
        "--dpi-desync-fooling=badseq",
        "--dpi-desync-autottl",
        "--dpi-desync-ttl=4",
        "--dpi-desync-fooling=datanoack",
    ] {
        out.push(format!("--dpi-desync=fake {f}"));
        out.push(format!("--dpi-desync=fake {f} --dpi-desync-repeats=6"));
        out.push(format!(
            "--dpi-desync=fake,multisplit --dpi-desync-split-pos=1 {f} --dpi-desync-fake-tls=$FAKE/tls_clienthello_www_google_com.bin"
        ));
    }
    out
}

/// Machine-generated candidates for autoselection.
pub fn catalog(engine: Engine, full: bool) -> Vec<Strategy> {
    let tls = match engine {
        Engine::Zapret2 => {
            if full {
                z2_full()
            } else {
                Z2_QUICK.iter().map(|s| s.to_string()).collect()
            }
        }
        Engine::Zapret => {
            if full {
                z1_full()
            } else {
                Z1_QUICK.iter().map(|s| s.to_string()).collect()
            }
        }
        Engine::Byedpi => Vec::new(),
    };
    tls.into_iter()
        .map(|t| {
            let template = match engine {
                Engine::Zapret2 => z2(&t),
                Engine::Zapret => z1(&t),
                Engine::Byedpi => t.clone(),
            };
            let mut s = Strategy::new(
                format!("cat-{}-{}", engine.id(), hash_id(&t)),
                engine,
                t.clone(),
                template,
            );
            s.source = Source::Catalog;
            s.description = Some(
                (if full {
                    "полный"
                } else {
                    "быстрый"
                })
                .to_string(),
            );
            s
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_not_empty() {
        assert!(catalog(Engine::Zapret2, false).len() >= 20);
        assert!(catalog(Engine::Zapret, false).len() >= 10);
    }
}
