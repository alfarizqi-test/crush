# Maintainer: Architect
pkgname=crush
pkgver=0.1.0
pkgrel=1
pkgdesc="A modern, fast, and unified shell written in Rust"
arch=('x86_64')
url="https://github.com/alfarizqi-test/crush"
license=('MIT')
depends=('gcc-libs')
makedepends=('cargo')

source=("git+file://${PWD}")
# source=("${pkgname}-${pkgver}.tar.gz::https://github.com/alfarizqi-test/crush/archive/refs/tags/v${pkgver}.tar.gz")
sha256sums=('SKIP') 

build() {
    # Hapus prefix versi karena kita mengambil dari git lokal
    cd "${pkgname}"
    cargo build --release --locked
}

package() {
    cd "${pkgname}"
    install -Dm755 "target/release/${pkgname}" "${pkgdir}/usr/bin/${pkgname}"
}

# build() {
#     cd "${pkgname}-${pkgver}"
#     cargo build --release --locked
# }

# package() {
#     cd "${pkgname}-${pkgver}"
    # Pastikan nama binary dari Cargo.toml memang 'crush' (huruf kecil)
#     install -Dm755 "target/release/${pkgname}" "${pkgdir}/usr/bin/${pkgname}"
# }
