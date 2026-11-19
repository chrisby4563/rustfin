import json, re, sys
spec=json.load(open(sys.argv[1])); p=sys.argv[2]; src=open(p).read()
schemas=spec['components']['schemas']
def ident(v, taken):
    parts=re.findall(r'[A-Z]+(?=[A-Z][a-z]|[0-9]|\b)|[A-Z]?[a-z]+|[A-Z]+|[0-9]+', v)
    i=''.join(x[:1].upper()+x[1:].lower() for x in parts) or 'X'
    if i[0].isdigit(): i='X'+i
    base=i; n=2
    while i in taken: i=f"{base}{n}"; n+=1
    return i
added=0; caught=0
for name in re.findall(r'\n    pub enum ([A-Za-z0-9_]+) \{', src):
    m=re.search(r'(\n    pub enum '+name+r' \{\n)(.*?)(\n    \}\n)', src, re.S)
    body=m.group(2)
    # nur reine String-Enums (Unit-Varianten)
    if re.search(r'^\s*[A-Za-z0-9_]+\s*[({]', body, re.M): continue
    have={}; ren=None; idents=set()
    for l in body.split('\n'):
        l=l.strip()
        r=re.match(r'#\[serde\(rename = "(.*)"\)\]',l)
        if r: ren=r.group(1); continue
        v=re.match(r'([A-Za-z0-9_]+),?$',l)
        if v: have[ren or v.group(1)]=v.group(1); idents.add(v.group(1)); ren=None
    new=[]
    sc=schemas.get(name,{})
    for val in sc.get('enum',[]):
        if val not in have:
            i=ident(val, idents); idents.add(i); new.append((val,i))
    add=''
    for val,i in new:
        add+=(f'\n        #[serde(rename = "{val}")]' if val!=i else '')+f'\n        {i},'
    if 'Unrecognized' not in idents:
        add+='\n        /// Vom Server geschickter Wert, den diese Bibliothek nicht kennt.\n        #[serde(other)]\n        Unrecognized,'
        caught+=1
    if not add: continue
    nb=body.rstrip(',')+','+add if not body.rstrip().endswith(',') else body+add
    src=src[:m.start(2)]+nb+src[m.end(2):]
    # ToString / FromStr Arme
    ts=re.search(r'(impl ToString for '+name+r' \{.*?match \*self \{\n)(.*?)(\n\s*\}\n\s*\}\n\s*\})', src, re.S)
    if ts:
        arms=''.join(f'\n                Self::{i} => "{val}".to_string(),' for val,i in new)
        if 'Unrecognized' in add: arms+='\n                Self::Unrecognized => "Unrecognized".to_string(),'
        src=src[:ts.end(2)]+arms+src[ts.end(2):]
    fs=re.search(r'(impl std::str::FromStr for '+name+r' \{.*?match value \{\n)(.*?)(\n\s*_ => Err)', src, re.S)
    if fs and new:
        arms=''.join(f'\n                "{val}" => Ok(Self::{i}),' for val,i in new)
        src=src[:fs.end(2)]+arms+src[fs.end(2):]
    added+=len(new)
open(p,'w').write(src)
print("neue Werte:",added,"Auffangwerte:",caught)
