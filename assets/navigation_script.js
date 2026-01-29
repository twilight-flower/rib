"use strict";

function pass_window_hash_to_section_iframe() {
	let section_iframe = document.getElementById("section");
	let window_hash = window.location.hash;
	section_iframe.contentWindow.location.hash = window_hash;
}

function focus_section_iframe() {
	let section_iframe = document.getElementById("section");
	section_iframe.contentWindow.focus();
}

window.onload = _ => {
	pass_window_hash_to_section_iframe;
	focus_section_iframe()
}
window.onhashchange = pass_window_hash_to_section_iframe;
