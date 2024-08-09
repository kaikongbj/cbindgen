use std::io::Write;

use crate::bindgen::{Bindings, cdecl, Config};
use crate::bindgen::DocumentationLength;
use crate::bindgen::ir::{ConditionWrite, Documentation, Enum, EnumVariant, Field, Function, Item, Literal, OpaqueItem, Static, Struct, to_known_assoc_constant, ToCondition, Type, Typedef, Union};
use crate::bindgen::language_backend::LanguageBackend;
use crate::bindgen::writer::{ListType, SourceWriter};

#[derive(Serialize, Clone)]
struct Pin {
    name: String,
    description: String,
    typ: String,
}
impl From<&Field> for Pin {
    fn from(f: &Field) -> Self {
        let config = Config::default();
        let d = cdecl::CDecl::from_type(&f.ty, &config);
        let typ = d.type_name.clone();
        Self {
            name: f.name.clone().parse().unwrap(),
            description: f.documentation.doc_comment.join("\n"),
            typ,
        }
    }
}
#[derive(Serialize, Clone)]
struct DATATYPE {
    name: String,
    description: String,
    pins: Vec<Pin>,
}
impl From<&Struct> for DATATYPE {
    fn from(s: &Struct) -> Self {
        Self {
            name: s.name().clone().parse().unwrap(),
            description: s.documentation.doc_comment.join("\n"),
            pins: s.fields.iter().map(Pin::from).collect(),
        }
    }
}
#[derive(Serialize, Clone)]
struct POU {
    name: String,
    description: String,
    params: Vec<String>,
    return_type: String,
}
impl From<&Function> for POU {
    fn from(f: &Function) -> Self {
        Self {
            name: f.path.to_string(),
            description: f.documentation.doc_comment.join("\n"),
            params: f.args.iter().map(|p| {
                let config = Config::default();
                let d = cdecl::CDecl::from_type(&p.ty, &config);
                d.type_name.clone()
            }).collect(),
            return_type: {
                let config = Config::default();
                let d = cdecl::CDecl::from_type(&f.ret, &config);
                d.type_name.clone()
            },
        }
    }
}
#[derive(Serialize)]
struct CombinedData {
    datatypes: Vec<DATATYPE>,
    pous: Vec<POU>,
}
pub struct POULanguageBackend<'a> {
    config: &'a Config,
    data: CombinedData,
    js: String,
}

impl<'a> POULanguageBackend<'a> {
    pub fn new(config: &'a Config) -> Self {
        Self { config, js: "".to_string(), data: CombinedData { datatypes: vec![], pous: vec![] } }
    }

    fn write_enum_variant<W: Write>(&mut self, out: &mut SourceWriter<W>, u: &EnumVariant) {
        self.write_documentation(out, &u.documentation);
        write!(out, "{}", u.export_name);
        if let Some(discriminant) = &u.discriminant {
            // For extern pou declarations the enumerator value is ignored,
            // but still useful as documentation, so we write it as a comment.
            out.write(" # = ");
            self.write_literal(out, discriminant);
        }
        out.write(",");
    }

    fn write_field<W: Write>(&mut self, out: &mut SourceWriter<W>, f: &Field) {
        // pou doesn't support conditional fields.
        // let condition = f.cfg.to_condition(self.config);

        self.write_documentation(out, &f.documentation);
        cdecl::write_field(self, out, &f.ty, &f.name, self.config);

        // pou extern declarations don't manage layouts, layouts are defined entierly by the
        // corresponding C code. So we can omit bitfield sizes which are not supported by pou.
    }
}

impl LanguageBackend for POULanguageBackend<'_> {
    fn open_namespaces<W: Write>(&mut self, out: &mut SourceWriter<W>) {
        // out.new_line();
        // let header = &self.config.pou.header.as_deref().unwrap_or("*");
        // write!(out, "cdef extern from {}", header);
        // out.open_brace();
    }

    fn close_namespaces<W: Write>(&mut self, out: &mut SourceWriter<W>) {

        // out.close_brace(false);
    }

    fn write_headers<W: Write>(&self, out: &mut SourceWriter<W>, package_version: &str) {
        if self.config.package_version {
            write!(out, "''' Package version: {} '''", package_version);
            out.new_line();
        }
        if let Some(ref f) = self.config.header {
            out.new_line_if_not_start();
            write!(out, "{}", f);
            out.new_line();
        }

        if self.config.include_version {
            out.new_line_if_not_start();
            write!(
                out,
                "/* Generated with poubindgen:{} */",
                crate::bindgen::config::VERSION
            );
            out.new_line();
        }
        if let Some(ref f) = &self.config.autogen_warning {
            out.new_line_if_not_start();
            write!(out, "{}", f);
            out.new_line();
        }

        if self.config.no_includes
            && self.config.sys_includes().is_empty()
            && self.config.includes().is_empty()
            && (self.config.pou.cimports.is_empty())
            && self.config.after_includes.is_none()
        {
            return;
        }

        out.new_line_if_not_start();

        // if !&self.config.no_includes {
        // out.write("from libc.stdint cimport int8_t, int16_t, int32_t, int64_t, intptr_t");
        // out.new_line();
        // out.write("from libc.stdint cimport uint8_t, uint16_t, uint32_t, uint64_t, uintptr_t");
        // out.new_line();
        // out.write("cdef extern from *");
        // out.open_brace();
        // out.write("ctypedef bint bool");
        // out.new_line();
        // out.write("ctypedef struct va_list");
        // out.new_line();
        // out.close_brace(false);
        // }

        for (module, names) in &self.config.pou.cimports {
            write!(out, "from {} cimport {}", module, names.join(", "));
            out.new_line();
        }

        if let Some(ref line) = &self.config.after_includes {
            write!(out, "{}", line);
            out.new_line();
        }
    }

    fn write_footers<W: Write>(&mut self, out: &mut SourceWriter<W>) {
        match serde_json::to_string(&self.data) {
            Ok(js) => {
                self.js = js;
                write!(out, "{}", self.js.as_str());
            }
            Err(e) => {
                println!("Error: {}", e);
            }
        }
    }

    fn write_enum<W: Write>(&mut self, out: &mut SourceWriter<W>, e: &Enum) {
        let size = e.repr.ty.map(|ty| ty.to_primitive().to_repr_c(self.config));
        let has_data = e.tag.is_some();
        let inline_tag_field = Enum::inline_tag_field(&e.repr);
        let tag_name = e.tag_name();

        let condition = e.cfg.to_condition(self.config);
        condition.write_before(self.config, out);

        self.write_documentation(out, &e.documentation);

        // Emit the tag enum and everything related to it.
        e.write_tag_enum(self.config, self, out, size, Self::write_enum_variant);

        // If the enum has data, we need to emit structs for the variants and gather them together.
        if has_data {
            e.write_variant_defs(self.config, self, out);
            out.new_line();
            out.new_line();

            e.open_struct_or_union(self.config, out, inline_tag_field);

            // Emit tag field that is separate from all variants.
            e.write_tag_field(self.config, out, size, inline_tag_field, tag_name);
            out.new_line();

            // Emit fields for all variants with data.
            e.write_variant_fields(self.config, self, out, inline_tag_field, Self::write_field);

            // Emit the post_body section, if relevant.
            if let Some(body) = &self.config.export.post_body(&e.path) {
                out.new_line();
                out.write_raw_block(body);
            }

            out.close_brace(true);
        }

        condition.write_after(self.config, out);
    }

    fn write_struct<W: Write>(&mut self, out: &mut SourceWriter<W>, s: &Struct) {
        let datatype = DATATYPE::from(s);
        self.data.datatypes.push(datatype);
    }

    fn write_union<W: Write>(&mut self, out: &mut SourceWriter<W>, u: &Union) {
        let condition = u.cfg.to_condition(self.config);
        condition.write_before(self.config, out);

        self.write_documentation(out, &u.documentation);

        out.write(self.config.style.pou_def());

        out.write("union");

        write!(out, " {}", u.export_name);

        out.open_brace();

        // Emit the pre_body section, if relevant
        if let Some(body) = &self.config.export.pre_body(&u.path) {
            out.write_raw_block(body);
            out.new_line();
        }

        out.write_vertical_source_list(self, &u.fields, ListType::Cap(";"), Self::write_field);
        if u.fields.is_empty() {
            out.write("pass");
        }

        // Emit the post_body section, if relevant
        if let Some(body) = &self.config.export.post_body(&u.path) {
            out.new_line();
            out.write_raw_block(body);
        }

        out.close_brace(true);

        condition.write_after(self.config, out);
    }

    fn write_opaque_item<W: Write>(&mut self, out: &mut SourceWriter<W>, o: &OpaqueItem) {
        let condition = o.cfg.to_condition(self.config);
        condition.write_before(self.config, out);

        self.write_documentation(out, &o.documentation);

        o.generic_params.write_with_default(self, self.config, out);

        write!(
            out,
            "{}struct {}",
            &self.config.style.pou_def(),
            o.export_name()
        );
        out.open_brace();
        out.write("pass");
        out.close_brace(false);

        condition.write_after(self.config, out);
    }

    fn write_type_def<W: Write>(&mut self, out: &mut SourceWriter<W>, t: &Typedef) {}

    fn write_static<W: Write>(&mut self, out: &mut SourceWriter<W>, s: &Static) {
        let condition = s.cfg.to_condition(self.config);
        condition.write_before(self.config, out);

        self.write_documentation(out, &s.documentation);
        out.write("extern ");
        if let Type::Ptr { is_const: true, .. } = s.ty {} else if !s.mutable {
            out.write("const ");
        }
        cdecl::write_field(self, out, &s.ty, &s.export_name, self.config);
        out.write(";");

        condition.write_after(self.config, out);
    }

    fn write_type<W: Write>(&mut self, out: &mut SourceWriter<W>, t: &Type) {
        cdecl::write_type(self, out, t, self.config);
    }

    fn write_documentation<W: Write>(&mut self, out: &mut SourceWriter<W>, d: &Documentation) {
        if d.doc_comment.is_empty() || !&self.config.documentation {
            return;
        }

        let end = match &self.config.documentation_length {
            DocumentationLength::Short => 1,
            DocumentationLength::Full => d.doc_comment.len(),
        };

        // pou uses Python-style comments, so `documentation_style` is not relevant.
        for line in &d.doc_comment[..end] {
            write!(out, "#{}", line);
            out.new_line();
        }
    }

    fn write_literal<W: Write>(&mut self, out: &mut SourceWriter<W>, l: &Literal) {
        match l {
            Literal::Expr(v) => match &**v {
                "true" => write!(out, "True"),
                "false" => write!(out, "False"),
                v => write!(out, "{}", v),
            },
            Literal::Path {
                ref associated_to,
                ref name,
            } => {
                if let Some((ref path, ref export_name)) = associated_to {
                    if let Some(known) = to_known_assoc_constant(path, name) {
                        return write!(out, "{}", known);
                    }
                    write!(out, "{}_", export_name)
                }
                write!(out, "{}", name)
            }
            Literal::FieldAccess {
                ref base,
                ref field,
            } => {
                write!(out, "(");
                self.write_literal(out, base);
                write!(out, ").{}", field);
            }
            Literal::PostfixUnaryOp { op, ref value } => {
                write!(out, "{}", op);
                self.write_literal(out, value);
            }
            Literal::BinOp {
                ref left,
                op,
                ref right,
            } => {
                write!(out, "(");
                self.write_literal(out, left);
                write!(out, " {} ", op);
                self.write_literal(out, right);
                write!(out, ")");
            }
            Literal::Cast { ref ty, ref value } => {
                out.write("<");
                self.write_type(out, ty);
                out.write(">");
                self.write_literal(out, value);
            }
            Literal::Struct {
                export_name,
                fields,
                path,
            } => {
                write!(out, "<{}>", export_name);

                write!(out, "{{ ");
                let mut is_first_field = true;
                // In C++, same order as defined is required.
                let ordered_fields = out.bindings().struct_field_names(path);
                for ordered_key in ordered_fields.iter() {
                    if let Some(lit) = fields.get(ordered_key) {
                        if !is_first_field {
                            write!(out, ", ");
                        } else {
                            is_first_field = false;
                        }
                        self.write_literal(out, lit);
                    }
                }
                write!(out, " }}");
            }
        }
    }

    fn write_functions<W: Write>(&mut self, out: &mut SourceWriter<W>, b: &Bindings) {
        for f in &b.functions {
            let pou = POU::from(f);
            self.data.pous.push(pou);
        }
    }
}
