import hashlib,json,sys
from pathlib import Path
role,checkout,destination=sys.argv[1:]
checkout=Path(checkout).resolve();destination=Path(destination).resolve()
pack=Path(__file__).resolve().parent
manifest=json.loads((pack/"manifest.json").read_text())
for file,digest in manifest["candidates"][role]["files_sha256"].items():
    assert hashlib.sha256((checkout/file).read_bytes()).hexdigest()==digest, file
assert not destination.exists(), "use a new owned test directory"
destination.mkdir(parents=True)
for file in ("Cargo.toml","Cargo.lock","lib.rs"):
    template=pack/(role+"-"+file+".template")
    if template.exists():
        (destination/file).write_text(template.read_text().replace("@CHECKOUT@",str(checkout)))
print("Prepared actual-module pure host:",role,destination)
