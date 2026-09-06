class Mantis < Formula
  desc "Fast terminal file tree viewer with syntax highlighting, markdown rendering, and fuzzy search"
  homepage "https://github.com/ansromanov/mantis"
  version "0.19.0"
  license "GPL-3.0-or-later"

  on_macos do
    on_arm do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-macos-aarch64"
      sha256 "e4d293f8c6776c5fd11325c52328d35d00ef64a0a8032a210c313f79af0c8c39"
    end

    on_intel do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-macos-x86_64"
      sha256 "7d167d4157797e1573dc015d649be411f2c588a53dae7834cc902c87c572e75b"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-linux-aarch64"
      sha256 "7c9ac70450b37f976aad1848f7c2d1247f38d22308f562743be0ff07dea99ba1"
    end

    on_intel do
      url "https://github.com/ansromanov/mantis/releases/download/v#{version}/mantis-linux-x86_64"
      sha256 "e889078920c15997eae95835e3500fb59384c82585e0f861b496f3abf490afe6"
    end
  end

  def install
    bin.install Dir["mantis-*"].first => "mantis"

    # Generate shell completions and man page from the installed binary.
    %w[bash zsh fish].each do |shell|
      (buildpath/"mantis.#{shell}").write Utils.popen_read(bin/"mantis", "--completions", shell)
    end
    bash_completion.install "mantis.bash"
    zsh_completion.install "mantis.zsh" => "_mantis"
    fish_completion.install "mantis.fish"

    (buildpath/"mantis.1").write Utils.popen_read(bin/"mantis", "--print-man-page")
    man1.install "mantis.1"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/mantis --version")
    # Completions and man page should be non-empty.
    assert_match "mantis", shell_output("#{bin}/mantis --completions bash")
    assert_match ".TH", shell_output("#{bin}/mantis --print-man-page")
  end
end
