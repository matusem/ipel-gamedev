class GamedevCli < Formula
  desc "UPJŠ GDD Platform developer CLI"
  homepage "https://github.com/matusem/ipel-gamedev"
  version "0.1.0"
  license "MIT"

  on_macos do
    on_intel do
      url "https://github.com/matusem/ipel-gamedev/releases/download/gamedev-cli-v0.1.0/gamedev-cli-macos-x86_64.tar.gz"
      sha256 "REPLACE_WITH_SHA256"
    end
    on_arm do
      url "https://github.com/matusem/ipel-gamedev/releases/download/gamedev-cli-v0.1.0/gamedev-cli-macos-aarch64.tar.gz"
      sha256 "REPLACE_WITH_SHA256"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/matusem/ipel-gamedev/releases/download/gamedev-cli-v0.1.0/gamedev-cli-linux-x86_64.tar.gz"
      sha256 "REPLACE_WITH_SHA256"
    end
    on_arm do
      url "https://github.com/matusem/ipel-gamedev/releases/download/gamedev-cli-v0.1.0/gamedev-cli-linux-aarch64.tar.gz"
      sha256 "REPLACE_WITH_SHA256"
    end
  end

  def install
    bin.install "gamedev" => "gamedev-cli"
  end

  test do
    assert_match "gamedev-cli", shell_output("#{bin}/gamedev-cli --version")
  end
end
