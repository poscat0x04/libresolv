/// interval type, representing the interval [pi_1, pi_2], pi_i should always be less than pi_2
type Interval = (u32, u32);
type Point = u32;

// Version range
enum Range {
    Interval(Interval),
    Point(Point),
    All,
}

struct Requirement<T> {
    package_name: T,
    versions: Vec<Range>,
}

struct PackageVer<T> {
    version: u32,
    dependencies: Vec<Requirement<T>>,
    conflicts: Vec<Requirement<T>>,
}

struct Package<T> {
    name: T,
    versions: Vec<PackageVer<T>>,
}

type Repository<T> = Vec<Package<T>>;