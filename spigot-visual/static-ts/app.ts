async function do_something() {
  console.log("hey");
}

window.onload = async (): Promise<void> => {
  await do_something();
};
