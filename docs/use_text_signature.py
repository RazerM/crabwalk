def autodoc_process_signature(
        app, what, name, obj, options, signature, return_annotation,
):
    if signature is None and hasattr(obj, '__text_signature__'):
        return obj.__text_signature__, return_annotation


def setup(app):
    app.connect("autodoc-process-signature", autodoc_process_signature)
