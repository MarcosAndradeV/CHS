fn is_print(c char) -> bool {
	return (c >= 32) && (c <= 126);
}

fn is_digit(c char) -> bool {
	return (c >= *"0") && (c <= *"9");
}
