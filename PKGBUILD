# Maintainer: Architect
pkgname=Crush
pkgver=0.1.0
pkgrel=1
pkgdesc="A modern, fast, and unified shell written in Rust"
arch=('x86_64')
url="https://github.com/alfarizqi-test/crush"
license=('MIT')
depends=('gcc-libs')
makedepends=('cargo')
install="${pkgname}.install" # Menghubungkan script .install di atas
source=("${pkgname}-${pkgver}.tar.gz::https://github.com/alfarizqi-test/crush/archive/refs/tags/v${pkgver}.tar.gz")
sha256sums=('SKIP') # Ganti dengan checksum aslinya nanti

build() {
    cd "${pkgname}-${pkgver}"
    # Build dengan mode release
    cargo build --release --locked
}

package() {
    cd "${pkgname}-${pkgver}"
    # Pindahkan binary hasil build ke /usr/bin/
    install -Dm755 "target/release/${pkgname}" "${pkgdir}/usr/bin/${pkgname}"
}
