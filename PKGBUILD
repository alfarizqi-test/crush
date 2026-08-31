# Maintainer: Architect
pkgname=crush
pkgver=0.1.0
pkgrel=1
pkgdesc="A modern, fast, and unified shell written in Rust"
arch=('x86_64')
url="https://github.com/alfarizqi-test/crush"
license=('MIT')
depends=('gcc-libs' 'glibc')
makedepends=('cargo')

# Mengambil langsung dari tarball rilis GitHub
source=("${pkgname}-${pkgver}.tar.gz::${url}/archive/refs/tags/v${pkgver}.tar.gz")
sha256sums=('SKIP') # Ganti 'SKIP' dengan hash SHA256 asli jika sudah final

build() {
    cd "${pkgname}-${pkgver}"
    # Opsi --locked memastikan cargo menggunakan versi dependency persis di Cargo.lock
    cargo build --release --locked
}

package() {
    cd "${pkgname}-${pkgver}"
    
    # Install binary crush ke /usr/bin/
    install -Dm755 "target/release/${pkgname}" "${pkgdir}/usr/bin/${pkgname}"
    
    # Wajib untuk Arch Linux: Install file lisensi MIT
    install -Dm644 LICENSE "${pkgdir}/usr/share/licenses/${pkgname}/LICENSE"
}