//! Main window.

use std::{cell::RefCell, rc::Rc};

use foxres_core::{Color, ESeries, bands::BandCount};
use relm4::{
    ComponentParts, ComponentSender, SimpleComponent,
    adw::{self, prelude::*},
    gtk,
};

use crate::{draw, text};

pub struct App {
    window: adw::ApplicationWindow,
    drawing: gtk::DrawingArea,
    bands_group: adw::PreferencesGroup,
    band_rows: Vec<adw::ComboRow>,
    count: BandCount,
    colors: Rc<RefCell<Vec<Color>>>,

    band_value: String,
    band_detail: String,
    value_hint: String,

    smd_value: String,
    smd_notation: String,
    smd_codes: String,

    series: ESeries,
    series_text: String,
    nearest_value: String,
    nearest_detail: String,

    combo_series: ESeries,
    combo_text: String,
    series_pair: String,
    parallel_pair: String,
}

#[derive(Debug)]
pub enum Msg {
    BandCount(u32),
    Band(usize, u32),
    ValueText(String),
    SmdCode(String),
    SmdValue(String),
    NearestText(String),
    NearestSeries(u32),
    ComboText(String),
    ComboSeries(u32),
    About,
}

const DEFAULT_SERIES: u32 = 3; // E24

#[relm4::component(pub)]
impl SimpleComponent for App {
    type Init = ();
    type Input = Msg;
    type Output = ();

    view! {
        #[root]
        adw::ApplicationWindow {
            set_title: Some("FoxRes"),
            set_default_size: (560, 760),

            adw::ToolbarView {
                add_top_bar = &adw::HeaderBar {
                    #[wrap(Some)]
                    set_title_widget = &adw::ViewSwitcher {
                        set_stack: Some(&stack),
                        set_policy: adw::ViewSwitcherPolicy::Wide,
                    },
                    pack_end = &gtk::Button {
                        set_icon_name: "help-about-symbolic",
                        set_tooltip_text: Some("About FoxRes"),
                        connect_clicked => Msg::About,
                    },
                },

                #[wrap(Some)]
                #[name = "stack"]
                set_content = &adw::ViewStack {
                    add_titled_with_icon[Some("bands"), "Color Code", "view-continuous-symbolic"] = &adw::PreferencesPage {
                        add = &adw::PreferencesGroup {
                            #[local_ref]
                            add = drawing -> gtk::DrawingArea {
                                set_content_height: 130,
                                set_hexpand: true,
                            },
                            add = &gtk::Label {
                                add_css_class: "title-1",
                                set_selectable: true,
                                #[watch]
                                set_label: &model.band_value,
                            },
                            add = &gtk::Label {
                                add_css_class: "dim-label",
                                set_wrap: true,
                                set_justify: gtk::Justification::Center,
                                #[watch]
                                set_label: &model.band_detail,
                            },
                        },
                        add = &adw::PreferencesGroup {
                            add = &adw::ComboRow {
                                set_title: "Number of bands",
                                set_model: Some(&gtk::StringList::new(&["3 bands", "4 bands", "5 bands", "6 bands"])),
                                set_selected: 1,
                                connect_selected_notify[sender] => move |row| {
                                    sender.input(Msg::BandCount(row.selected()));
                                },
                            },
                            add = &adw::EntryRow {
                                set_title: "Or type a value (4k7, 220, 1M)",
                                connect_changed[sender] => move |e| {
                                    sender.input(Msg::ValueText(e.text().to_string()));
                                },
                            },
                            add = &gtk::Label {
                                add_css_class: "caption",
                                add_css_class: "error",
                                set_margin_top: 6,
                                set_wrap: true,
                                #[watch]
                                set_visible: !model.value_hint.is_empty(),
                                #[watch]
                                set_label: &model.value_hint,
                            },
                        },
                        #[local_ref]
                        add = bands_group -> adw::PreferencesGroup {
                            set_title: "Bands",
                            set_description: Some("Read from the end farthest from the gap"),
                        },
                    },

                    add_titled_with_icon[Some("smd"), "SMD", "view-grid-symbolic"] = &adw::PreferencesPage {
                        add = &adw::PreferencesGroup {
                            set_title: "Decode a marking",
                            add = &adw::EntryRow {
                                set_title: "Code",
                                connect_changed[sender] => move |e| {
                                    sender.input(Msg::SmdCode(e.text().to_string()));
                                },
                            },
                            add = &gtk::Label {
                                add_css_class: "title-1",
                                set_margin_top: 12,
                                set_selectable: true,
                                #[watch]
                                set_label: &model.smd_value,
                            },
                            add = &gtk::Label {
                                add_css_class: "dim-label",
                                #[watch]
                                set_label: &model.smd_notation,
                            },
                        },
                        add = &adw::PreferencesGroup {
                            set_title: "Find the marking for a value",
                            add = &adw::EntryRow {
                                set_title: "Resistance",
                                connect_changed[sender] => move |e| {
                                    sender.input(Msg::SmdValue(e.text().to_string()));
                                },
                            },
                            add = &gtk::Label {
                                set_margin_top: 12,
                                set_selectable: true,
                                set_wrap: true,
                                #[watch]
                                set_label: &model.smd_codes,
                            },
                        },
                    },

                    add_titled_with_icon[Some("eseries"), "E-Series", "view-list-symbolic"] = &adw::PreferencesPage {
                        add = &adw::PreferencesGroup {
                            set_title: "Nearest standard value",
                            add = &adw::EntryRow {
                                set_title: "Target resistance",
                                connect_changed[sender] => move |e| {
                                    sender.input(Msg::NearestText(e.text().to_string()));
                                },
                            },
                            add = &adw::ComboRow {
                                set_title: "Series",
                                set_model: Some(&gtk::StringList::new(&series_names)),
                                set_selected: DEFAULT_SERIES,
                                connect_selected_notify[sender] => move |row| {
                                    sender.input(Msg::NearestSeries(row.selected()));
                                },
                            },
                            add = &gtk::Label {
                                add_css_class: "title-1",
                                set_margin_top: 12,
                                set_selectable: true,
                                #[watch]
                                set_label: &model.nearest_value,
                            },
                            add = &gtk::Label {
                                add_css_class: "dim-label",
                                set_wrap: true,
                                #[watch]
                                set_label: &model.nearest_detail,
                            },
                        },
                    },

                    add_titled_with_icon[Some("combine"), "Combine", "list-add-symbolic"] = &adw::PreferencesPage {
                        add = &adw::PreferencesGroup {
                            set_title: "Two-resistor combinations",
                            set_description: Some("Best pair of standard values for a value you don't stock"),
                            add = &adw::EntryRow {
                                set_title: "Target resistance",
                                connect_changed[sender] => move |e| {
                                    sender.input(Msg::ComboText(e.text().to_string()));
                                },
                            },
                            add = &adw::ComboRow {
                                set_title: "Series",
                                set_model: Some(&gtk::StringList::new(&series_names)),
                                set_selected: DEFAULT_SERIES,
                                connect_selected_notify[sender] => move |row| {
                                    sender.input(Msg::ComboSeries(row.selected()));
                                },
                            },
                        },
                        add = &adw::PreferencesGroup {
                            add = &adw::ActionRow {
                                set_title: "Series",
                                add_css_class: "property",
                                #[watch]
                                set_subtitle: &model.series_pair,
                            },
                            add = &adw::ActionRow {
                                set_title: "Parallel",
                                add_css_class: "property",
                                #[watch]
                                set_subtitle: &model.parallel_pair,
                            },
                        },
                    },
                },
            },
        }
    }

    fn init(_: (), root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let series_names: Vec<&str> = ESeries::ALL.iter().map(|s| s.name()).collect();
        let initial = vec![Color::Yellow, Color::Violet, Color::Red, Color::Gold];
        let (band_value, band_detail) = text::bands(&initial);
        let series = ESeries::ALL[DEFAULT_SERIES as usize];
        let (_, nearest_detail) = text::nearest("", series);
        let (_, smd_notation) = text::smd_code("");

        let mut model = App {
            window: root.clone(),
            drawing: gtk::DrawingArea::new(),
            bands_group: adw::PreferencesGroup::new(),
            band_rows: Vec::new(),
            count: BandCount::Four,
            colors: Rc::new(RefCell::new(initial)),
            band_value,
            band_detail,
            value_hint: String::new(),
            smd_value: String::new(),
            smd_notation,
            smd_codes: String::new(),
            series,
            series_text: String::new(),
            nearest_value: String::new(),
            nearest_detail,
            combo_series: series,
            combo_text: String::new(),
            series_pair: String::new(),
            parallel_pair: String::new(),
        };

        let colors = model.colors.clone();
        model.drawing.set_draw_func(move |_, cr, w, h| {
            draw::resistor(cr, w, h, &colors.borrow());
        });

        let drawing = &model.drawing;
        let bands_group = &model.bands_group;
        let widgets = view_output!();

        model.rebuild_band_rows(&sender);
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Msg, sender: ComponentSender<Self>) {
        match msg {
            Msg::BandCount(i) => {
                let Some(&count) = BandCount::CHOICES.get(i as usize) else {
                    return;
                };
                self.change_band_count(count);
                self.rebuild_band_rows(&sender);
            }
            Msg::Band(pos, idx) => {
                let role = self.count.roles()[pos];
                if let Some(&c) = role.choices().get(idx as usize) {
                    if let Some(slot) = self.colors.borrow_mut().get_mut(pos) {
                        *slot = c;
                    }
                    self.value_hint.clear();
                    self.refresh_bands();
                }
            }
            Msg::ValueText(t) => {
                if t.trim().is_empty() {
                    self.value_hint.clear();
                    return;
                }
                let (tol, tc) = self.tolerance_and_tempco();
                match text::value_to_bands(&t, self.count, tol, tc) {
                    Ok(colors) if colors.len() == self.count.len() => {
                        *self.colors.borrow_mut() = colors;
                        self.value_hint.clear();
                        self.rebuild_band_rows(&sender);
                    }
                    Ok(_) => self.value_hint = "0 Ω is a single black band".into(),
                    Err(e) => self.value_hint = e,
                }
            }
            Msg::SmdCode(t) => (self.smd_value, self.smd_notation) = text::smd_code(&t),
            Msg::SmdValue(t) => self.smd_codes = text::smd_for_value(&t),
            Msg::NearestText(t) => {
                self.series_text = t;
                self.refresh_nearest();
            }
            Msg::NearestSeries(i) => {
                self.series = ESeries::ALL.get(i as usize).copied().unwrap_or(self.series);
                self.refresh_nearest();
            }
            Msg::ComboText(t) => {
                self.combo_text = t;
                self.refresh_pairs();
            }
            Msg::ComboSeries(i) => {
                self.combo_series = ESeries::ALL
                    .get(i as usize)
                    .copied()
                    .unwrap_or(self.combo_series);
                self.refresh_pairs();
            }
            Msg::About => self.show_about(),
        }
    }
}

impl App {
    fn tolerance_and_tempco(&self) -> (Color, Color) {
        use foxres_core::bands::Role;
        let colors = self.colors.borrow();
        let find = |role: Role| {
            self.count
                .roles()
                .iter()
                .position(|&r| r == role)
                .and_then(|i| colors.get(i).copied())
        };
        let tol = find(Role::Tolerance).unwrap_or(match self.count {
            BandCount::Five | BandCount::Six => Color::Brown,
            _ => Color::Gold,
        });
        (tol, find(Role::Tempco).unwrap_or(Color::Brown))
    }

    fn change_band_count(&mut self, count: BandCount) {
        let current = foxres_core::bands::decode(&self.colors.borrow()).map(|r| r.ohms);
        let (tol, tc) = self.tolerance_and_tempco();
        self.count = count;
        let tol = if count == BandCount::Four && tol.tolerance().unwrap_or(0.0) < 1.0 {
            Color::Gold
        } else {
            tol
        };
        let encoded = current
            .ok()
            .and_then(|ohms| foxres_core::bands::encode(ohms, count, tol, tc).ok())
            .filter(|c| c.len() == count.len());
        let default = match count {
            BandCount::Three => vec![Color::Yellow, Color::Violet, Color::Red],
            BandCount::Four | BandCount::One => {
                vec![Color::Yellow, Color::Violet, Color::Red, Color::Gold]
            }
            BandCount::Five => vec![
                Color::Yellow,
                Color::Violet,
                Color::Black,
                Color::Brown,
                Color::Brown,
            ],
            BandCount::Six => vec![
                Color::Yellow,
                Color::Violet,
                Color::Black,
                Color::Brown,
                Color::Brown,
                Color::Brown,
            ],
        };
        *self.colors.borrow_mut() = encoded.unwrap_or(default);
        self.value_hint.clear();
    }

    /// Recreate the per-band dropdowns to match `count` and `colors`.
    fn rebuild_band_rows(&mut self, sender: &ComponentSender<Self>) {
        for row in self.band_rows.drain(..) {
            self.bands_group.remove(&row);
        }
        let colors = self.colors.borrow().clone();
        for (pos, &role) in self.count.roles().iter().enumerate() {
            let choices = role.choices();
            let names: Vec<&str> = choices.iter().map(|c| c.name()).collect();
            let row = adw::ComboRow::builder()
                .title(format!("Band {}", pos + 1))
                .subtitle(role.label())
                .model(&gtk::StringList::new(&names))
                .build();
            let selected = colors
                .get(pos)
                .and_then(|c| choices.iter().position(|x| x == c))
                .unwrap_or(0);
            row.set_selected(selected as u32);
            let sender = sender.clone();
            row.connect_selected_notify(move |r| sender.input(Msg::Band(pos, r.selected())));
            self.bands_group.add(&row);
            self.band_rows.push(row);
        }
        self.refresh_bands();
    }

    fn refresh_bands(&mut self) {
        (self.band_value, self.band_detail) = text::bands(&self.colors.borrow());
        self.drawing.queue_draw();
    }

    fn refresh_nearest(&mut self) {
        (self.nearest_value, self.nearest_detail) = text::nearest(&self.series_text, self.series);
    }

    fn refresh_pairs(&mut self) {
        (self.series_pair, self.parallel_pair) = text::pairs(&self.combo_text, self.combo_series);
    }

    fn show_about(&self) {
        let about = adw::AboutDialog::builder()
            .application_name("FoxRes")
            .application_icon(crate::APP_ID)
            .developer_name("FoxIQ")
            .version(env!("CARGO_PKG_VERSION"))
            .website("https://www.foxiq.io")
            .issue_url("https://github.com/richardnero/FoxRes/issues")
            .license_type(gtk::License::MitX11)
            .copyright("© 2026 FoxIQ")
            .comments("Resistor color codes, SMD markings, E-series values and combinations.")
            .build();
        about.present(Some(&self.window));
    }
}
