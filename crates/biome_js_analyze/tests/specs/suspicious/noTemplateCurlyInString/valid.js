/* should not generate diagnostics */
let a = `Hello, ${name}`;
let a = templateFunction`Hello, ${name}`;
let a = `Hello, name`;
let a = 'Hello, name';
let a = 'Hello, ' + name;
let a = `Hello, ${index + 1}`;
let a = `Hello, ${name + " foo"}`;
let a = `Hello, ${name || "foo"}`;
let a = `Hello, ${{foo: "bar"}.foo}`;
let a = '$2';
let a = '${';
let a = '$}';
let a = '{foo}';
let a = '{foo: "bar"}';