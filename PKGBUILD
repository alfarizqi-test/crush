pkgname=crush-bin
pkgver=0.1.0
pkgrel=1
pkgdesc="The lightweight modern custom shell written in Rust (Pre-compiled)"
arch=('x86_64')
url="https://github.com/alfarizqi-test/crush"
license=('MIT')
provides=('crush')
conflicts=('crush')

install="crush.install"

source=("${url}/releases/download/v${pkgver}/crush-x86_64-linux")
sha256sums=('SKIP')

package() {
    install -Dm755 "${srcdir}/crush-x86_64-linux" "${pkgdir}/usr/bin/crush"
}