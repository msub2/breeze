open Bogue
open Gopher
open History
open Protocols
open Networking

(* Window size constants *)
let _width = ref 640
let _height = ref 480

let go_action gopher_view urlbar = 
  let url = Widget.get_text urlbar in
  History.add_entry (url, Gopher);
  let (host, port, selector) = parse_gopher_url url in
  let request_body = selector ^ "\r\n" in
  let response = network_request host port request_body in
  parse_gopher_response response gopher_view urlbar

let history_action (action : history_action) gopher_view urlbar = 
  let can_navigate = match action with 
  | Forward -> History.can_go_forward ()
  | Back -> History.can_go_backward () in

  if can_navigate then
    let _ = match action with 
    | Forward -> History.history_forward ()
    | Back -> History.history_back () in
    let (url, pagetype) = History.get_history () in
    let (host, port, selector) = parse_gopher_url url in
    let request_body = selector ^ "\r\n" in
    let response = network_request host port request_body in
    let _ = match pagetype with
    | Gopher -> parse_gopher_response response gopher_view urlbar
    | Plaintext -> parse_plaintext_response response gopher_view
    | _ -> parse_plaintext_response response gopher_view in
    
    Widget.set_text urlbar url

(* Main Loop *)
let () =
  Theme.set_text_font "./Inconsolata.ttf";
  let gopherview_widget = Widget.text_display "" in
  let gopher_view = gopherview_widget
    |> Layout.resident ~w:!_width ~h:!_height
    |> Layout.make_clip ~w:!_width ~h:!_height in
  let urlbar = Widget.text_input ~text:"gopher.floodgap.com" ~prompt:"Enter URL..." () ~size:16 in
  let go_button = Widget.button "Go" ~action:(fun _ -> go_action gopher_view urlbar) in
  let back_button = Widget.button "<" ~action:(fun _ -> history_action Back gopher_view urlbar) in
  let forward_button = Widget.button ">" ~action:(fun _ -> history_action Forward gopher_view urlbar) in
  let toolbar = Layout.flat_of_w [back_button; forward_button; urlbar; go_button] ~background:(Layout.color_bg (Draw.transp Draw.grey)) in
  Layout.set_width toolbar !_width;
  go_action gopher_view urlbar;

  [toolbar; gopher_view]
    |> Layout.tower ~name:"Breeze - A SmolNet Browser"
    |> Bogue.of_layout
    |> Bogue.run