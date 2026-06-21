# Makefile.am for ngspice-doc
#
# Copyright (C) Holger Vogt
#           (C) 2018 Carsten Schoenert <c.schoenert@...>
#
# This file is part of the ngspice-doc. Please see README and COPYING for
# terms of use.
#
# SPDX-License-Identifier:        CC-BY-SA-4.0

## Additional targets that will be processed by the distcheck target.
installcheck-am: check-lyx-files check-png-files

# dist
EXTRA_DIST  = manual.lyx Tables Images

MAINTAINERCLEANFILES = manual.pdf

## Needed PNG files for the documentation.
GRAPHIC_FILES = \
 $(top_srcdir)/Images/C4.png \
 $(top_srcdir)/Images/C5.png \
 $(top_srcdir)/Images/CIDER_Conc_profile1.png \
 $(top_srcdir)/Images/CIDER_Conc_profile2.png \
 $(top_srcdir)/Images/Example_Circuit_1.png \
 $(top_srcdir)/Images/Example_Circuit_2.png \
 $(top_srcdir)/Images/Example_Circuit_C1.png \
 $(top_srcdir)/Images/Example_Circuit_C2.png \
 $(top_srcdir)/Images/Example_Circuit_C3.png \
 $(top_srcdir)/Images/fft_windows.png \
 $(top_srcdir)/Images/Filter-IO.png \
 $(top_srcdir)/Images/gnuplot-stat-an.png \
 $(top_srcdir)/Images/mesh-3a.png \
 $(top_srcdir)/Images/ng-win-out.png \
 $(top_srcdir)/Images/ng-win-out-white.png \
 $(top_srcdir)/Images/optim-speed.png \
 $(top_srcdir)/Images/Subc-filter.png \
 $(top_srcdir)/Images/vr2-trrandom.png \
 $(top_srcdir)/Images/XSPICE-Toplevel.png

## Needed LYX files for the documentation.
LYX_FILES = \
 $(top_srcdir)/Tables/ASRC.lyx \
 $(top_srcdir)/Tables/BJT2.lyx \
 $(top_srcdir)/Tables/BJT.lyx \
 $(top_srcdir)/Tables/BSIM1.lyx \
 $(top_srcdir)/Tables/BSIM2.lyx \
 $(top_srcdir)/Tables/Capacitor.lyx \
 $(top_srcdir)/Tables/CCCS.lyx \
 $(top_srcdir)/Tables/CCVS.lyx \
 $(top_srcdir)/Tables/CplLines.lyx \
 $(top_srcdir)/Tables/Inductor.lyx \
 $(top_srcdir)/Tables/Isource.lyx \
 $(top_srcdir)/Tables/LTRA.lyx \
 $(top_srcdir)/Tables/MOS1.lyx \
 $(top_srcdir)/Tables/MOS2.lyx \
 $(top_srcdir)/Tables/MOS3.lyx \
 $(top_srcdir)/Tables/MOS6.lyx \
 $(top_srcdir)/Tables/MOS9.lyx \
 $(top_srcdir)/Tables/Mutual.lyx \
 $(top_srcdir)/Tables/Resistor.lyx \
 $(top_srcdir)/Tables/Tranline.lyx \
 $(top_srcdir)/Tables/TransLine.lyx \
 $(top_srcdir)/Tables/URC.lyx \
 $(top_srcdir)/Tables/VBIC.lyx \
 $(top_srcdir)/Tables/VCCS.lyx \
 $(top_srcdir)/Tables/VCVS.lyx \
 $(top_srcdir)/Tables/Vsource.lyx

BUILD_PDF = build_pdf
BUILD_HTML = build_html

all: manual.pdf manual.html

manual.pdf:
	@$(MKDIR_P) $(top_builddir)/$(BUILD_PDF) ;\
		echo "building $(top_builddir)/$(BUILD_PDF)/manual.pdf" ;\
		lyx @LYX_USER_DIR@ -E pdf2 $(BUILD_PDF)/manual.pdf $(top_srcdir)/manual.lyx

manual.html:
	@$(MKDIR_P) $(top_builddir)/$(BUILD_HTML) ;\
		echo "building $(top_builddir)/$(BUILD_HTML)/manual.xhtml" ;\
		lyx @LYX_USER_DIR@ -E xhtml $(BUILD_HTML)/manual.html $(top_srcdir)/manual.lyx

install-pdf-am: manual.pdf
	$(MKDIR_P) $(DESTDIR)$(docdir) || exit 1
	cp -a -f $(top_builddir)/$(BUILD_PDF)/manual.pdf $(DESTDIR)$(docdir)

install-html-am: manual.html
	$(MKDIR_P) $(DESTDIR)$(docdir)/html || exit 1
	cp -a -f $(top_builddir)/$(BUILD_HTML)/* $(DESTDIR)$(htmldir)/html

## Some sanity checks for the LYX files.
## This target will get called by the 'distcheck' target, The intention is
## to check for completeness of the files referenced by $(LYX_FILES).
check-lyx-files:
	@echo -e "\nChecking availability of listed files in \$$(LYX_FILES)..."
	@for file in $(LYX_FILES); do \
		NOT_FOUND=0 ;\
		if [ ! -f $${file} ]; then \
			echo $${file} not found! ;\
			NOT_FOUND=1 ;\
		fi \
	done ;\
	if [ "$${NOT_FOUND}" -eq "1" ]; then \
		echo "At least one file from \$$(LYX_FILES) could not be found!" ;\
	else \
		echo "All files found." ;\
	fi
	@echo -e "\nChecking for existing files that are *not* listed in \$$(LYX_FILES)..."
	@FOUND_LYX_FILES=`find $(top_srcdir)/Tables -type f -name "*.lyx" | sort` ;\
	ERROR=0 ;\
	for entry in $${FOUND_LYX_FILES}; do \
		FOUND_FILE=`basename $${entry}` ;\
		echo "Checking $${FOUND_FILE} from $(top_srcdir)/Tables" ;\
		for listed_lyx_file in $(LYX_FILES); do \
			GIVEN_FILE=`basename $${listed_lyx_file}` ;\
			FOUND=0 ;\
			if [ "$${FOUND_FILE}" == "$${GIVEN_FILE}" ]; then \
				FOUND=1 ;\
				break ;\
			fi ;\
		done ;\
		if [ "$${FOUND}" == "0" ]; then \
			echo "Ooops! '$${entry}' not found in \$$(LYX_FILES)!" ;\
			ERROR=1 ;\
		fi ;\
	done ;\
	if [ "$${ERROR}" == "1" ]; then \
		echo "" ;\
		echo -e "    \033[1;93mWARNING!!!" ;\
		echo "    At least one '*.lyx' file was found in the folder '$(top_srcdir)/Tables' that is not added to \$$(LYX_FILES) in '$(top_srcdir)/Makefile.am'!" ;\
		echo -e "    You might have forgotten to add these files?\e[0m" ;\
		echo "" ;\
	else \
		echo "All files found." ;\
	fi

## Some sanity checks for the PNG files.
## This target will get called by the 'distcheck' target, The intention is
## to check for completeness of the files referenced by $(GRAPHIC_FILES).
check-png-files:
	@echo -e "\nChecking availability of listed files in \$$(GRAPHIC_FILES)..."
	@for file in $(GRAPHIC_FILES); do \
		NOT_FOUND=0 ;\
		if [ ! -f $${file} ]; then \
			echo $${file} not found! ;\
			NOT_FOUND=1 ;\
		fi \
	done ;\
	if [ "$${NOT_FOUND}" -eq "1" ]; then \
		echo "At least one file from \$$(GRAPHICS_FILES) could not be found!" ;\
	else \
		echo "All files found." ;\
	fi
	@echo -e "\nChecking for existing files that are *not* listed in \$$(GRAPHICS_FILES)..."
	@FOUND_GRAPHICS_FILES=`find $(top_srcdir)/Images -type f -name "*.png" | sort` ;\
	ERROR=0 ;\
	for entry in $${FOUND_GRAPHICS_FILES}; do \
		FOUND_FILE=`basename $${entry}` ;\
		echo "Checking $${FOUND_FILE} from $(top_srcdir)/Tables" ;\
		for listed_lyx_file in $(GRAPHICS_FILES); do \
			GIVEN_FILE=`basename $${listed_graphics_file}` ;\
			FOUND=0 ;\
			if [ "$${FOUND_FILE}" == "$${GIVEN_FILE}" ]; then \
				FOUND=1 ;\
				break ;\
			fi ;\
		done ;\
		if [ "$${FOUND}" == "0" ]; then \
			echo "Ooops! '$${entry}' not found in \$$(GRAPHICS_FILES)!" ;\
			ERROR=1 ;\
		fi ;\
	done ;\
	if [ "$${ERROR}" == "1" ]; then \
		echo "" ;\
		echo -e "    \033[1;93mWARNING!!!" ;\
		echo "    At least one '*.png' file was found in the folder '$(top_srcdir)/Images' that is not added to \$$(GRAPHICS_FILES) in '$(top_srcdir)/Makefile.am'!" ;\
		echo -e "    You might have forgotten to add these files?\e[0m" ;\
		echo "" ;\
	else \
		echo "All files found." ;\
	fi

install-data-am: install-pdf-am install-html-am

uninstall-am:
	rm -rf $(DESTDIR)$(docdir) $(DESTDIR)$(htmldir)

clean-local:
	rm -rf $(top_builddir)/$(BUILD_PDF)
	rm -rf $(top_builddir)/$(BUILD_HTML)

distclean-local: clean-local

upload:
	chmod 664 $(top_builddir)/$(BUILD_PDF)/manual.pdf
	scp $(top_builddir)/$(BUILD_PDF)/manual.pdf web.sourceforge.net:htdocs/docs
	echo "ls -l htdocs/docs" | sftp web.sourceforge.net
