# Homebrew formula for a tap (github.com/Aphrodine-wq/homebrew-tap), not
# homebrew-core. Fill in the sha256 values from the release assets:
#   shasum -a 256 termpaper-*.tar.gz
class Termpaper < Formula
  desc "Wallpaper Engine for the terminal: live GPU-rendered scenes"
  homepage "https://github.com/Aphrodine-wq/termpaper"
  version "0.2.0"
  license "MIT"

  on_macos do
    # one universal binary for Apple Silicon and Intel
    url "https://github.com/Aphrodine-wq/termpaper/releases/download/v0.2.0/termpaper-universal-apple-darwin.tar.gz"
    sha256 "FILL-IN-AT-RELEASE"
  end

  on_linux do
    on_intel do
      url "https://github.com/Aphrodine-wq/termpaper/releases/download/v0.2.0/termpaper-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "FILL-IN-AT-RELEASE"
    end
    on_arm do
      url "https://github.com/Aphrodine-wq/termpaper/releases/download/v0.2.0/termpaper-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "FILL-IN-AT-RELEASE"
    end
  end

  def install
    bin.install "termpaper"
  end

  test do
    assert_match "termpaper", shell_output("#{bin}/termpaper --version")
  end
end
