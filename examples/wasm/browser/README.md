# Drogue-device WASM example

This example demonstrates running drogue-device in the browser using wasm_bindgen async runner.

It uses the Button and Led actors from drogue-device, which interacts with underlying HTML elements.

## Running

The simplest way to try this example is to install [`wasm-pack`](), `python3` and run the following
command:

```
./build.sh
```

This will spin up an HTTP server locally where you should see a button and the led state changing
whenever you click the button.
