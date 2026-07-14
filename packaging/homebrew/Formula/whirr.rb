class Whirr < Formula
  desc "System-tray player for internet radio (MP3) streams"
  homepage "https://github.com/samuelb/whirr"
  version "0.5.5"
  license "MIT"

  on_macos do
    url "https://github.com/samuelb/whirr/releases/download/v#{version}/whirr-macos-universal.tar.gz"
    # Release automation replaces these placeholders with the published checksums.
    sha256 "REPLACE_WITH_MACOS_UNIVERSAL_SHA256"
  end

  on_linux do
    on_arm do
      url "https://github.com/samuelb/whirr/releases/download/v#{version}/whirr-linux-arm64.tar.gz"
      sha256 "REPLACE_WITH_LINUX_ARM64_SHA256"
    end
    on_intel do
      url "https://github.com/samuelb/whirr/releases/download/v#{version}/whirr-linux-amd64.tar.gz"
      sha256 "REPLACE_WITH_LINUX_AMD64_SHA256"
    end
  end

  def install
    bin.install "whirr"
  end
end
