
use criterion::{black_box, criterion_group, criterion_main, Criterion, Bencher};
use nanorand::{Rng, WyRand};

trait TFoo
{
    fn method(&self, rng: &mut WyRand) -> i32;
}

struct Bar(i32);
impl TFoo for Bar { fn method(&self, rng: &mut WyRand) -> i32 { self.0 * rng.generate_range(0..20) } }
struct Baz(i64);
impl TFoo for Baz { fn method(&self, rng: &mut WyRand) -> i32 { (self.0 * rng.generate_range(0..50)) as i32 } }
struct Bonk(i16);
impl TFoo for Bonk { fn method(&self, rng: &mut WyRand) -> i32 { (self.0 * rng.generate_range(10..130)) as i32 } }
struct Quux(i8);
impl TFoo for Quux { fn method(&self, rng: &mut WyRand) -> i32 { (self.0 * rng.generate_range(-34..127)) as i32 } }

static bar: Bar = Bar(38);
static baz: Baz = Baz(1280479048);
static bonk: Bonk = Bonk(680);
static quux: Quux = Quux(3);

fn criterion_benchmark(c: &mut Criterion)
{
    c.bench_function("stacked", bench_stacked);
    c.bench_function("looped", bench_looped);
}

fn bench_stacked(b: &mut Bencher)
{
    let mut rng = WyRand::new();

    b.iter(|| {
        black_box(bar.method(&mut rng));
        black_box(baz.method(&mut rng));
        black_box(bonk.method(&mut rng));
        black_box(quux.method(&mut rng));
    });
}

fn bench_looped(b: &mut Bencher)
{
    let mut rng = WyRand::new();
    let zoop: [&dyn TFoo; 4] = [&bar, &baz, &bonk, &quux];

    b.iter(|| {
        for z in zoop
        {
            black_box(z.method(&mut rng));
        }
    });
}

criterion_group!(hlists, criterion_benchmark);
criterion_main!(hlists);
