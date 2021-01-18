use criterion::{criterion_group, criterion_main, Criterion};
use msgbus::{Message, MsgBus};

#[tokio::main]
pub async fn bm_onesender(c: &mut Criterion) {
    let (_, mut mbh) = MsgBus::<String, usize>::new();
    let mut rx = mbh.register("reg_token".into()).await.unwrap();
    tokio::spawn(async move {
        while let Some(m) = rx.recv().await {
            match m {
                Message::Shutdown => {
                    break;
                }
                _ => {}
            }
        }
    });

    c.bench_function("one sender", |b| {
        b.iter(|| {
            let mut mbh = mbh.clone();
            async move {
                mbh.send("reg_token".into(), 100).await.unwrap();
            }
        })
    });
}

#[tokio::main]
pub async fn bm_broadcast(c: &mut Criterion) {
    let (_, mut mbh) = MsgBus::<String, usize>::new();
    let mut rx = mbh.register("reg_token".into()).await.unwrap();
    tokio::spawn(async move {
        while let Some(m) = rx.recv().await {
            match m {
                Message::Shutdown => {
                    break;
                }
                _ => {}
            }
        }
    });

    c.bench_function("one broadcast", |b| {
        b.iter(|| {
            let mut mbh = mbh.clone();
            async move {
                mbh.broadcast(100).await.unwrap();
            }
        })
    });
}

#[tokio::main]
pub async fn bm_multibroadcast(c: &mut Criterion) {
    let (_, mut mbh) = MsgBus::<String, usize>::new();
    let mut rx = mbh.register("reg_token".into()).await.unwrap();
    let mut rx2 = mbh.register("reg_token2".into()).await.unwrap();

    tokio::spawn(async move {
        while let Some(m) = rx.recv().await {
            match m {
                Message::Shutdown => {
                    break;
                }
                _ => {}
            }
        }
    });
    tokio::spawn(async move {
        while let Some(m) = rx2.recv().await {
            match m {
                Message::Shutdown => {
                    break;
                }
                _ => {}
            }
        }
    });

    c.bench_function("multi broadcast", |b| {
        b.iter(|| {
            let mut mbh = mbh.clone();
            async move {
                mbh.broadcast(100).await.unwrap();
            }
        })
    });
}

#[tokio::main]
pub async fn bm_purempsc(c: &mut Criterion) {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<usize>(1);

    tokio::spawn(async move { while let Some(_) = rx.recv().await {} });

    c.bench_function("pure mpsc send", |b| {
        b.iter(|| {
            let tx = tx.clone();
            async move {
                tx.send(100).await.unwrap();
            }
        })
    });
}

criterion_group!(
    benches,
    bm_onesender,
    bm_broadcast,
    bm_multibroadcast,
    bm_purempsc
);
criterion_main!(benches);
