## 3. Module context and name resolution

A `bench ModName` is rooted at the elaborated `ModName`. Names resolve: (1) bench-local `var`s
and fn params; (2) the module's POM — its nets, instances, params. So `vsrc`, `resistor`, `sw`
are the module's; `resistor.p` is an instance port net; `sw.ctrl` a param/port. `gnd`/`Ground`
is the reference node. These node references are what a result's `.v`/`.i` take (§4). Post
monomorphization, generics appear in concrete form.

---

