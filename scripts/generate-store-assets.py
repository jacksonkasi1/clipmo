"""Generate Microsoft Store/MSIX assets from Clipmo's existing master icon."""

from pathlib import Path

from PIL import Image


REPO_ROOT = Path(__file__).resolve().parent.parent
SOURCE = REPO_ROOT / "src-tauri" / "icons" / "icon.png"
OUTPUT = REPO_ROOT / "store" / "Assets"


def write_square(source: Image.Image, name: str, size: int) -> None:
    source.resize((size, size), Image.Resampling.LANCZOS).save(
        OUTPUT / name, format="PNG", optimize=True
    )


def main() -> None:
    source = Image.open(SOURCE).convert("RGBA")
    OUTPUT.mkdir(parents=True, exist_ok=True)

    write_square(source, "StoreLogo.png", 50)
    write_square(source, "Square44x44Logo.png", 44)
    write_square(source, "Square150x150Logo.png", 150)

    wide = Image.new("RGBA", (310, 150), (0, 0, 0, 0))
    logo = source.resize((120, 120), Image.Resampling.LANCZOS)
    wide.alpha_composite(logo, ((310 - 120) // 2, (150 - 120) // 2))
    wide.save(OUTPUT / "Wide310x150Logo.png", format="PNG", optimize=True)


if __name__ == "__main__":
    main()
