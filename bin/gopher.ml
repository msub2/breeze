open Bogue
open History
open Networking

(* Window size constants *)
let _width = ref 640
let _height = ref 480

type gopher_line = {
  line_kind : char;
  text : string;
  selector : string;
  server : string;
  port : int;
};;

let new_gopher_line line_kind text selector server port = 
  { line_kind; text; selector; server; port }

let parse_gopher_url url =
  match String.split_on_char '/' url with
  | host :: selector_parts -> 
      let request_body = String.concat "/" selector_parts in
      (host, 70, request_body)
  | [] -> failwith "Invalid URL"

let build_gopher_line line =
  let chunks = String.split_on_char '\t' line in
  if (List.length chunks >= 4) then
    let both = List.nth chunks 0 in
    let line_kind = String.get both 0 in
    let text = String.sub both 1 (String.length both - 1) in
    let gopher_line = new_gopher_line
      line_kind text (List.nth chunks 1) (List.nth chunks 2) (Option.value (int_of_string_opt (List.nth chunks 3)) ~default:70) in
    gopher_line
  else
    new_gopher_line 'i' "" "" "" 70

let trim_leading_slash s =
  if String.length s > 0 && s.[0] = '/' then
    String.sub s 1 (String.length s - 1)
  else
    s

let parse_plaintext_response response gopher_view =
  let height = String.split_on_char '\n' response
    |> List.length in
  let text = Widget.text_display response
    |> Layout.resident ~w:!_width ~h:(height * 18)
    |> Layout.make_clip ~scrollbar:false ~w:!_width ~h:!_height in

  Layout.set_rooms gopher_view [text]

let get_icon line_kind = 
  match line_kind with
    | '0' -> "file-text"
    | '1' -> "folder-open"
    | _ -> "question"

let rec parse_gopher_response response gopher_view urlbar = 
  let tokens = String.split_on_char '\n' response in
  let lines = List.map build_gopher_line tokens in
  let style_line line =
    let line_widgets = match line.line_kind with
    | '0' | '1' -> 
      let icon = get_icon line.line_kind |> Widget.icon in
      let text = Widget.rich_text [(Text_display.underline (Text_display.raw line.text))] ~w:!_width ~h:18 in
      Widget.mouse_over ~enter:(fun _ -> Draw.set_system_cursor Tsdl.Sdl.System_cursor.hand) text;
      let on_click _ =
        String.concat "/" [line.server; trim_leading_slash line.selector]
          |> Widget.set_text urlbar;
        let request_body = line.selector ^ "\r\n" in
        let response = network_request line.server line.port request_body in
        match line.line_kind with
        | '0' -> 
          History.add_entry (Widget.get_text urlbar, Plaintext);
          parse_plaintext_response response gopher_view
        | '1' -> 
          History.add_entry (Widget.get_text urlbar, Gopher);
          parse_gopher_response response gopher_view urlbar
        | _ -> () in (* Unreachable *)
      Widget.on_click ~click:on_click text;
      [icon; text]
    | '7' -> 
      let text = Widget.text_display line.text in
      let search_field = Widget.text_input () in
      let search_action _ =
        line.server ^ line.selector ^ "\t" ^ Widget.get_text search_field ^ "\r\n"
          |> Widget.set_text urlbar;
        let request_body = line.selector ^ "\t" ^ Widget.get_text search_field ^ "\r\n" in
        let response = network_request line.server line.port request_body in
        History.add_entry (Widget.get_text urlbar, Gopher);
        parse_gopher_response response gopher_view urlbar in
      let go_button = Widget.button ~action:(fun _ -> search_action ())"Go" in
      [text; search_field; go_button]
    | 'i' -> [Widget.text_display line.text ~w:!_width ~h:18]
    | _ -> 
      let icon = Widget.icon "question" in
      let text = Widget.rich_text [(Text_display.italic (Text_display.raw line.text))] ~w:!_width ~h:18 in
      [icon; text] in
    Layout.flat_of_w line_widgets ~sep:0 in
  let widgets = List.map style_line lines
    |> Layout.tower
    |> Layout.make_clip ~scrollbar:false ~w:!_width ~h:(!_height - 0) in

  Layout.set_rooms gopher_view [widgets]